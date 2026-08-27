use super::{
    Arc, Duration, RuntimeServer, RuntimeServerEndpoint, RuntimeServerExit, RuntimeServerState,
    WorkspaceDbRegistry, cleanup_runtime_server_endpoint, fixture_endpoint,
    prepare_runtime_server_endpoint, publish_runtime_server_endpoint, runtime_server_endpoint_path,
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

#[tokio::test(flavor = "multi_thread")]
async fn endpoint_is_published_only_after_every_required_transport_plane_is_ready() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let endpoint_path = runtime_dir.path().join("runtime-server-endpoint.v1.json");
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server control plane");
    let _client_grpc_listener = tokio::net::UnixListener::bind(&endpoint.data_plane_socket_path)
        .expect("bind ASP Client gRPC plane");
    let _provider_listener = tokio::net::UnixListener::bind(&endpoint.provider_plane_socket_path)
        .expect("bind provider plane");
    server
        .publish_endpoint_after_required_planes(&endpoint_path)
        .await
        .expect("publish ready Runtime Server endpoint");

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
async fn failed_ready_endpoint_publication_removes_every_bound_runtime_artifact() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 31).await;
    let blocking_parent = runtime_dir.path().join("not-a-directory");
    tokio::fs::write(&blocking_parent, b"block directory creation")
        .await
        .expect("write blocking parent");
    let endpoint_path = blocking_parent.join("runtime-server-endpoint.v1.json");

    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server control plane");
    let _client_grpc_listener = tokio::net::UnixListener::bind(&endpoint.data_plane_socket_path)
        .expect("bind ASP Client gRPC plane");
    let _provider_listener = tokio::net::UnixListener::bind(&endpoint.provider_plane_socket_path)
        .expect("bind provider plane");
    let error = server
        .publish_endpoint_after_required_planes(&endpoint_path)
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
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
            &runtime_digest,
        )
        .expect("typed runtime digest"),
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
