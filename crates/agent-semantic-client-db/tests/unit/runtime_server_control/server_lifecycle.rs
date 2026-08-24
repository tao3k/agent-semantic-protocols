use super::{
    Arc, Duration, RuntimeServer, RuntimeServerControlRequest, RuntimeServerEndpoint,
    RuntimeServerExit, RuntimeServerOperation, RuntimeServerState, WorkspaceDbRegistry,
    call_runtime_server, cleanup_runtime_server_endpoint, fixture_endpoint,
    prepare_runtime_server_endpoint, publish_runtime_server_endpoint, runtime_server_endpoint_path,
    runtime_server_status_memory_metrics,
};

#[tokio::test(flavor = "current_thread")]
async fn runtime_server_readiness_watch_starts_starting_and_publishes_healthy() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 901).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint,
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let mut readiness = server.readiness_subscribe();
    assert_eq!(*readiness.borrow(), RuntimeServerState::Starting);
    let shutdown = server.shutdown_handle();
    let task = tokio::spawn(server.serve());
    tokio::time::timeout(Duration::from_secs(2), readiness.changed())
        .await
        .expect("readiness watch must publish without polling")
        .expect("readiness sender must remain alive");
    assert_eq!(*readiness.borrow(), RuntimeServerState::Healthy);
    shutdown.shutdown();
    task.await
        .expect("join Runtime Server")
        .expect("serve Runtime Server")
        .eq(&RuntimeServerExit::ShutdownRequested)
        .then_some(())
        .expect("Runtime Server must shut down cleanly");
}

#[tokio::test]
async fn shutdown_aborts_a_connection_that_does_not_observe_drain() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 2).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());
    let _stalled_connection = tokio::net::UnixStream::connect(&endpoint.data_plane_socket_path)
        .await
        .expect("open a data-plane connection that never sends a frame");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let started = tokio::time::Instant::now();
    shutdown.shutdown();
    let exit = tokio::time::timeout(Duration::from_millis(250), server)
        .await
        .expect("Runtime Server shutdown must not wait forever on a stuck connection")
        .expect("join Runtime Server")
        .expect("serve Runtime Server");

    assert_eq!(exit, RuntimeServerExit::ShutdownRequested);
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "stuck connection drain exceeded the shutdown boundary: {:?}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_endpoint_omits_workspace_and_process_identity() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, _artifact_catalog) = fixture_endpoint(&runtime_dir, 21).await;
    let value = serde_json::to_value(&endpoint).expect("encode runtime server endpoint");
    let object = value.as_object().expect("endpoint JSON object");

    assert!(!object.contains_key("workspaceIdentity"));
    assert!(!object.contains_key("ownerPid"));
    assert!(endpoint.socket_path.len() <= 103);
    assert_eq!(
        agent_semantic_client_db::runtime_server_endpoint_path(runtime_dir.path())
            .expect("derive Runtime Server endpoint path"),
        agent_semantic_client_db::runtime_server_runtime_base(runtime_dir.path())
            .expect("derive UID-scoped Runtime Server base")
            .join("endpoint.v1.json")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_restart_is_not_downgraded_to_status_for_the_current_digest() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, _artifact_catalog) = fixture_endpoint(&runtime_dir, 22).await;
    let request = RuntimeServerControlRequest {
        schema_id: "agent.semantic-protocols.runtime-server-control-request".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Restart,
        project_root: None,
        expected_runtime_binary_identity: endpoint.runtime_binary_identity.clone(),
        request_id: "restart-idempotency".to_owned(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
    };

    assert!(
        request
            .requires_restart(&endpoint)
            .expect("validate explicit restart")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn source_index_lease_miss_is_fail_fast_and_never_opens_turso() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_root = runtime_dir.path().join("query-miss-workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create query project root");
    let workspace_identity = "workspace-query-miss".to_owned();
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 32).await;
    let durable_registry = Arc::new(WorkspaceDbRegistry::with_state_home(
        &runtime_dir.path().join("state"),
    ));
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::clone(&durable_registry),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let memory_registry = Arc::clone(server.workspace_registry());
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());
    let session =
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity,
            project_root.clone(),
        );
    let request = agent_semantic_client_db::workspace_db_ipc::WorkspaceDbSourceIndexLookupRequest {
        project_root: project_root.clone(),
        indexed_project_root: project_root,
        query: "missing-generation".to_owned(),
        language_id: Some("rust".into()),
        limit: 8,
    };

    session
        .health()
        .await
        .expect("establish the Runtime data-plane connection before timing the lease miss");
    let started = tokio::time::Instant::now();
    let error = session
        .read_source_index(&request)
        .await
        .expect_err("query without an admitted generation must fail closed");
    assert!(
        error.contains("active workspace generation lease is required"),
        "unexpected query miss error: {error}"
    );
    assert!(
        started.elapsed() < Duration::from_millis(10),
        "generation lease miss must fail in milliseconds: {:?}",
        started.elapsed()
    );
    let entry_counts = durable_registry.workspace_entry_counts();
    assert_eq!(entry_counts.slot_count, 0);
    assert_eq!(entry_counts.loaded_entry_count, 0);
    let counters = memory_registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.schema_bootstraps, 0);
    assert_eq!(counters.lock_probes, 0);

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn typed_status_and_restart_use_the_real_runtime_server() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 23).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

    let prewarm_status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "prewarm-status-mmap".to_owned(),
    )
    .await
    .expect("prewarm Runtime Server status mmap");
    assert_eq!(prewarm_status.state, RuntimeServerState::Healthy);

    let started = tokio::time::Instant::now();
    let status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "status-1".to_owned(),
    )
    .await
    .expect("call runtime server status");
    assert_eq!(status.state, RuntimeServerState::Healthy);

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            value: "next-runtime-digest".to_owned(),
            algorithm: "blake3-256".to_owned(),
        },
        "restart-1".to_owned(),
    )
    .await
    .expect("call runtime server restart");
    assert_eq!(restart.state, RuntimeServerState::Draining);
    assert!(
        started.elapsed() < Duration::from_millis(10),
        "two warm runtime server roundtrips exceeded 10ms: {:?}",
        started.elapsed()
    );
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
    let draining_status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "post-restart-status".to_owned(),
    )
    .await
    .expect("read draining status from mapped snapshot after server exit");
    assert_eq!(draining_status.state, RuntimeServerState::Draining);
    let draining_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("draining status memory metrics");
    assert_eq!(draining_metrics.reader_open_count, 1);
    assert_eq!(draining_metrics.snapshot_decode_count, 2);
    assert!(draining_metrics.snapshot_cache_hit_count >= 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn endpoint_is_published_only_after_both_runtime_server_sockets_are_ready() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let endpoint_path = runtime_dir.path().join("runtime-server-endpoint.v1.json");
    let server = RuntimeServer::bind_and_publish_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        &endpoint_path,
        artifact_catalog,
    )
    .await
    .expect("bind and publish Runtime Server");

    assert!(
        tokio::fs::try_exists(&endpoint.socket_path)
            .await
            .expect("inspect control socket")
    );
    assert!(
        tokio::fs::try_exists(&endpoint.data_plane_socket_path)
            .await
            .expect("inspect data-plane socket")
    );
    assert!(
        tokio::fs::try_exists(&endpoint.status_memory_path)
            .await
            .expect("inspect status memory")
    );
    let published: RuntimeServerEndpoint = serde_json::from_slice(
        &tokio::fs::read(&endpoint_path)
            .await
            .expect("read published endpoint"),
    )
    .expect("decode published endpoint");
    assert_eq!(published, endpoint);

    drop(server);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_endpoint_publication_removes_every_bound_runtime_artifact() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 31).await;
    let blocking_parent = runtime_dir.path().join("not-a-directory");
    tokio::fs::write(&blocking_parent, b"block directory creation")
        .await
        .expect("write blocking parent");
    let endpoint_path = blocking_parent.join("runtime-server-endpoint.v1.json");

    let error = RuntimeServer::bind_and_publish_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        &endpoint_path,
        artifact_catalog,
    )
    .await
    .err()
    .expect("endpoint publication must fail");
    assert!(
        error.contains("failed to create Runtime Server endpoint directory"),
        "unexpected endpoint publication failure: {error}"
    );
    assert!(
        !tokio::fs::try_exists(&endpoint.socket_path)
            .await
            .expect("inspect control socket")
    );
    assert!(
        !tokio::fs::try_exists(&endpoint.data_plane_socket_path)
            .await
            .expect("inspect data-plane socket")
    );
    assert!(
        !tokio::fs::try_exists(&endpoint.status_memory_path)
            .await
            .expect("inspect status memory")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn owner_bound_cleanup_removes_provider_plane_socket() {
    let state_home = tempfile::tempdir().expect("create isolated Runtime Server State Home");
    let runtime_digest = format!("blake3-256:{}", "2".repeat(64));
    let catalog_digest = format!("blake3-256:{}", "1".repeat(64));
    let endpoint = prepare_runtime_server_endpoint(
        state_home.path(),
        std::path::Path::new("/runtime/asp"),
        &runtime_digest,
        "dev",
        &catalog_digest,
        91,
        "cleanup-binding",
    )
    .await
    .expect("prepare strict Runtime Server endpoint");
    for path in [
        &endpoint.socket_path,
        &endpoint.data_plane_socket_path,
        &endpoint.provider_plane_socket_path,
        &endpoint.status_memory_path,
    ] {
        tokio::fs::write(path, b"bound")
            .await
            .expect("create bound artifact");
    }
    let endpoint_path = runtime_server_endpoint_path(state_home.path())
        .expect("derive Runtime Server endpoint receipt path");
    publish_runtime_server_endpoint(&endpoint_path, &endpoint)
        .await
        .expect("publish endpoint receipt");

    cleanup_runtime_server_endpoint(state_home.path(), &endpoint)
        .await
        .expect("cleanup owner-bound endpoint artifacts");
    assert!(
        !tokio::fs::try_exists(&endpoint.provider_plane_socket_path)
            .await
            .expect("inspect provider-plane socket")
    );
    assert!(
        !tokio::fs::try_exists(&endpoint_path)
            .await
            .expect("inspect endpoint receipt")
    );
}
