use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, RuntimeServerOperation, RuntimeServerState,
    call_runtime_server, prepare_runtime_server_endpoint_in, prewarm_runtime_server_status_memory,
    runtime_server_status_memory_metrics, runtime_server_transport_contract_digest,
};

fn record_admission_fixture_candidate(project_root: &std::path::Path) {
    let digest = format!(
        "blake3:{}",
        blake3::hash(project_root.as_os_str().as_encoded_bytes()).to_hex()
    );
    agent_semantic_client_db::runtime_server_admission::record_workspace_generation_candidate(
        project_root.to_path_buf(),
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
            candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
                algorithm: "blake3-worktree-state-v1".to_owned(),
                digest: digest.clone(),
                authorities: vec![
                    agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex,
                ],
            },
            policy_overlay_digest: digest,
        },
    )
    .expect("record atomic admission fixture candidate");
}

#[test]
fn runtime_transport_identity_binds_control_and_workspace_data_plane_contracts() {
    let domain = b"agent.semantic-protocols.runtime-server-transport.v1";
    let control = include_bytes!("../../../../schemas/runtime-server-control.v1.schema.json");
    let data_plane = include_bytes!("../../../../schemas/workspace-db-owner-ipc.v1.schema.json");
    let performance_observation =
        include_bytes!("../../../../schemas/runtime-server-performance-observation.v1.schema.json");
    let performance_ingress_receipt = include_bytes!(
        "../../../../schemas/runtime-server-performance-ingress-receipt.v1.schema.json"
    );
    let mut expected = blake3::Hasher::new();
    expected.update(domain);
    for (contract_name, contract_bytes) in [
        (b"runtime-server-control.v1".as_slice(), control.as_slice()),
        (
            b"workspace-db-owner-ipc.v1".as_slice(),
            data_plane.as_slice(),
        ),
        (
            b"runtime-server-performance-observation.v1".as_slice(),
            performance_observation.as_slice(),
        ),
        (
            b"runtime-server-performance-ingress-receipt.v1".as_slice(),
            performance_ingress_receipt.as_slice(),
        ),
    ] {
        expected.update(&(contract_name.len() as u64).to_le_bytes());
        expected.update(contract_name);
        expected.update(&(contract_bytes.len() as u64).to_le_bytes());
        expected.update(contract_bytes);
    }
    let expected = format!("blake3-256:{}", expected.finalize().to_hex());

    assert_eq!(runtime_server_transport_contract_digest(), expected);
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(control).to_hex()),
        "control-only identity would admit an incompatible workspace data plane"
    );
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(data_plane).to_hex()),
        "data-plane-only identity would admit an incompatible control plane"
    );
}

pub(super) async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    let catalog = agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
        &state_home,
    )
    .await
    .expect("load runtime artifact catalog");
    let endpoint = prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        catalog.mode_label(),
        &catalog.digest(),
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint");
    (endpoint, Arc::new(catalog))
}

async fn concurrent_runtime_status_wave(
    endpoint: &RuntimeServerEndpoint,
    request_count: usize,
    request_prefix: &'static str,
) -> Vec<Duration> {
    let barrier = Arc::new(tokio::sync::Barrier::new(request_count + 1));
    let mut clients = tokio::task::JoinSet::new();
    for index in 0..request_count {
        let endpoint = endpoint.clone();
        let barrier = Arc::clone(&barrier);
        clients.spawn(async move {
            barrier.wait().await;
            let started = tokio::time::Instant::now();
            let receipt = call_runtime_server(
                &endpoint,
                RuntimeServerOperation::Status,
                endpoint.runtime_artifact_digest.clone(),
                format!("{request_prefix}-{index}"),
            )
            .await
            .expect("call concurrent runtime server");
            (receipt, started.elapsed())
        });
    }
    barrier.wait().await;

    let mut latencies = Vec::with_capacity(request_count);
    while let Some(completed) = clients.join_next().await {
        let (receipt, latency) = completed.expect("join concurrent client");
        assert_eq!(receipt.state, RuntimeServerState::Healthy);
        latencies.push(latency);
    }
    latencies.sort_unstable();
    latencies
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
        schema_id: "agent.semantic-protocols.runtime-server-control-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Restart,
        project_root: None,
        expected_runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
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

    let prewarm = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Reconcile,
        endpoint.runtime_artifact_digest.clone(),
        "prewarm-control-lane".to_owned(),
    )
    .await
    .expect("prewarm persistent Runtime Server control lane");
    assert_eq!(prewarm.state, RuntimeServerState::Healthy);
    let prewarm_status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "prewarm-status-mmap".to_owned(),
    )
    .await
    .expect("prewarm Runtime Server status mmap");
    assert_eq!(prewarm_status.state, RuntimeServerState::Healthy);

    let started = tokio::time::Instant::now();
    let status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "status-1".to_owned(),
    )
    .await
    .expect("call runtime server status");
    assert_eq!(status.state, RuntimeServerState::Healthy);

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        "next-runtime-digest".to_owned(),
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
        endpoint.runtime_artifact_digest.clone(),
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

#[tokio::test(flavor = "multi_thread")]
async fn adaptive_concurrent_runtime_control_is_sub_millisecond_at_p99() {
    let _performance = crate::test_support::performance_lock();
    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(16)
        .clamp(32, 512);
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 24).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

    let admission_started = tokio::time::Instant::now();
    let admission_metrics = prewarm_runtime_server_status_memory(&endpoint)
        .await
        .expect("admit Runtime Server status mapping before request execution");
    let admission_latency = admission_started.elapsed();
    assert_eq!(admission_metrics.reader_open_count, 1);
    assert_eq!(admission_metrics.snapshot_decode_count, 1);

    let cold_latencies =
        concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-expand").await;
    let p99_index = request_count.saturating_mul(99).div_ceil(100) - 1;
    let cold_p99 = cold_latencies[p99_index];
    assert!(
        cold_p99 < Duration::from_millis(1),
        "{request_count} concurrent cold query requests exceeded sub-millisecond p99: {cold_p99:?}"
    );
    let cold_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("cold status memory metrics");
    assert_eq!(cold_metrics.reader_open_count, 1);
    assert!(
        cold_metrics.snapshot_decode_count
            <= admission_metrics.snapshot_decode_count.saturating_add(1),
        "the starting-to-healthy transition may decode one new generation, but no more: \
         admission={admission_metrics:?} cold={cold_metrics:?}"
    );

    let latencies = concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-warm").await;
    let p99 = latencies[p99_index];
    assert!(
        p99 < Duration::from_millis(1),
        "{request_count} concurrent warm runtime requests exceeded sub-millisecond p99: {p99:?}"
    );
    let warm_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("warm status memory metrics");
    assert_eq!(warm_metrics.reader_open_count, 1);
    assert_eq!(
        warm_metrics.snapshot_decode_count, cold_metrics.snapshot_decode_count,
        "a stable healthy generation must not be decoded again"
    );
    assert_eq!(
        warm_metrics.snapshot_cache_hit_count - cold_metrics.snapshot_cache_hit_count,
        request_count as u64,
        "every warm Status request must use the mapped snapshot cache"
    );
    eprintln!(
        "runtime-server-performance requestCount={request_count} admissionMicros={} coldP99Micros={} \
         warmP99Micros={} readerOpens={} snapshotDecodes={} warmSnapshotCacheHits={}",
        admission_latency.as_micros(),
        cold_p99.as_micros(),
        p99.as_micros(),
        warm_metrics.reader_open_count,
        warm_metrics.snapshot_decode_count,
        warm_metrics.snapshot_cache_hit_count - cold_metrics.snapshot_cache_hit_count,
    );

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        "next-runtime-digest".to_owned(),
        "pressure-drain".to_owned(),
    )
    .await
    .expect("drain runtime server fixture");
    assert_eq!(restart.state, RuntimeServerState::Draining);
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
    let draining_status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "post-restart-status".to_owned(),
    )
    .await
    .expect("read draining status from mapped snapshot after server exit");
    assert_eq!(draining_status.state, RuntimeServerState::Draining);
    let draining_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("draining status memory metrics");
    assert_eq!(draining_metrics.reader_open_count, 1);
    assert_eq!(
        draining_metrics.snapshot_decode_count,
        warm_metrics.snapshot_decode_count.saturating_add(1),
        "the explicit healthy-to-draining transition must decode exactly one new generation"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn adaptive_persistent_control_lanes_have_bounded_typical_and_hard_latency() {
    let _performance = crate::test_support::performance_lock();
    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(4)
        .clamp(16, 128);
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 26).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

    let prewarm = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Reconcile,
        endpoint.runtime_artifact_digest.clone(),
        "prewarm-persistent-control".to_owned(),
    )
    .await
    .expect("prewarm persistent control lane");
    assert_eq!(prewarm.state, RuntimeServerState::Healthy);

    let mut tasks = Vec::with_capacity(request_count);
    for request in 0..request_count {
        let endpoint = endpoint.clone();
        tasks.push(tokio::spawn(async move {
            let started = tokio::time::Instant::now();
            let receipt = call_runtime_server(
                &endpoint,
                RuntimeServerOperation::Reconcile,
                endpoint.runtime_artifact_digest.clone(),
                format!("persistent-control-{request}"),
            )
            .await
            .expect("call persistent Runtime Server control lane");
            assert_eq!(receipt.state, RuntimeServerState::Healthy);
            started.elapsed()
        }));
    }
    let mut latencies = Vec::with_capacity(request_count);
    for task in tasks {
        latencies.push(task.await.expect("join persistent control request"));
    }
    latencies.sort_unstable();
    let p75 = latencies[request_count.saturating_mul(75).div_ceil(100) - 1];
    let max = *latencies.last().expect("persistent control samples");
    assert!(
        p75 < Duration::from_millis(50),
        "{request_count} persistent control requests exceeded 50ms at p75: {p75:?}"
    );
    assert!(
        max < Duration::from_millis(500),
        "{request_count} persistent control requests exceeded the 500ms hard boundary: {max:?}"
    );
    eprintln!(
        "runtime-server-persistent-control requestCount={request_count} p75Micros={} maxMicros={}",
        p75.as_micros(),
        max.as_micros()
    );

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        "next-runtime-digest".to_owned(),
        "persistent-control-drain".to_owned(),
    )
    .await
    .expect("drain Runtime Server");
    assert_eq!(restart.state, RuntimeServerState::Draining);
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
}
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_tokio_shutdown_drains_the_runtime_server_once() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 27).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let mut callers = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let shutdown = shutdown.clone();
        callers.spawn(async move {
            shutdown.shutdown();
        });
    }
    while let Some(result) = callers.join_next().await {
        result.expect("join concurrent Runtime Server shutdown caller");
    }

    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    let draining = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "post-shutdown-status".to_owned(),
    )
    .await
    .expect("read draining status after Tokio shutdown");
    assert_eq!(draining.state, RuntimeServerState::Draining);
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_generation_mutation_submission_is_non_blocking_and_single_flight() {
    let _performance = crate::test_support::performance_lock();
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_fixture = crate::test_support::TestDir::new("single-flight-admission");
    let project_root = project_fixture.path().join("project");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create admission project");
    record_admission_fixture_candidate(&project_root);
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
            .expect("resolve admission workspace")
            .workspace
            .workspace_id
            .to_string();
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 28).await;
    let source_build_count = Arc::new(tokio::sync::Mutex::new(0_u32));
    let source_build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let state_home = runtime_dir.path().join("state");
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_builder(Arc::new({
        let source_build_count = Arc::clone(&source_build_count);
        let source_build_release = Arc::clone(&source_build_release);
        move |_workspace_identity, _project_root, _changed_paths, _provider_target| {
            let source_build_count = Arc::clone(&source_build_count);
            let source_build_release = Arc::clone(&source_build_release);
            Box::pin(async move {
                *source_build_count.lock().await += 1;
                source_build_release
                    .acquire_owned()
                    .await
                    .map_err(|_| "fixture generation release closed".to_owned())?
                    .forget();
                Err("fixture stops before source publication".to_owned())
            })
        }
    }));
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());
    let session = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity,
            project_root.clone(),
        ),
    );
    session
        .health()
        .await
        .expect("establish the Runtime data-plane connection before timing submissions");

    let parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let request_count = parallelism.saturating_mul(32).max(64);
    let changed_path = project_root
        .join("src/lib.rs")
        .to_string_lossy()
        .into_owned();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..request_count {
        let session = Arc::clone(&session);
        let changed_path = changed_path.clone();
        requests.spawn(async move {
            let started = tokio::time::Instant::now();
            (
                session
                    .submit_runtime_generation_mutation(
                        "mutation-single-flight",
                        vec![changed_path],
                    )
                    .await,
                started.elapsed(),
            )
        });
    }
    let mut queued_count = 0_usize;
    let mut latencies = Vec::with_capacity(request_count);
    while let Some(result) = requests.join_next().await {
        let (receipt, latency) = result.expect("join Runtime mutation submission");
        let receipt = receipt.expect("Runtime mutation submission receipt");
        receipt
            .validate()
            .expect("valid mutation admission receipt");
        assert_eq!(receipt.mutation_id, "mutation-single-flight");
        queued_count += usize::from(matches!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
        ));
        latencies.push(latency);
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "runtime-server-single-workspace-submission requestCount={request_count} p99Micros={}",
        p99.as_micros()
    );
    assert_eq!(queued_count, 1);
    assert!(
        p99 < Duration::from_millis(10),
        "Runtime mutation submission IPC p99 exceeded 10ms: {p99:?}"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while *source_build_count.lock().await == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background mutation submission must reach the Runtime Server");

    let successor_b = session
        .submit_runtime_generation_mutation("mutation-successor-b", vec![changed_path.clone()])
        .await
        .expect("queue first successor mutation");
    let successor_c = session
        .submit_runtime_generation_mutation("mutation-successor-c", vec![changed_path.clone()])
        .await
        .expect("queue second successor mutation");
    let duplicate_b = session
        .submit_runtime_generation_mutation("mutation-successor-b", vec![changed_path.clone()])
        .await
        .expect("coalesce duplicate mutation already present in the active chain");
    assert!(matches!(
        successor_b.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
    ));
    assert!(matches!(
        successor_c.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
    ));
    assert!(matches!(
        duplicate_b.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Coalesced
    ));

    source_build_release.add_permits(3);
    tokio::time::timeout(Duration::from_millis(100), async {
        while *source_build_count.lock().await < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the resident writer lane must start all queued generation attempts within 100ms");
    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    assert_eq!(*source_build_count.lock().await, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multi_workspace_multi_session_admission_is_single_flight_and_bounded() {
    let _performance = crate::test_support::performance_lock();
    const WORKSPACE_COUNT: usize = 3;
    const SESSION_COUNT_PER_WORKSPACE: usize = 12;
    const CALL_COUNT_PER_SESSION: usize = 16;
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_fixture = crate::test_support::TestDir::new("multi-workspace-admission");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 30).await;
    let source_build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let source_build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let state_home = runtime_dir.path().join("state");
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(&state_home)),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_builder(Arc::new({
        let source_build_count = Arc::clone(&source_build_count);
        let source_build_release = Arc::clone(&source_build_release);
        move |_workspace_identity, _project_root, _changed_paths, _provider_target| {
            let source_build_count = Arc::clone(&source_build_count);
            let source_build_release = Arc::clone(&source_build_release);
            Box::pin(async move {
                source_build_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                source_build_release
                    .acquire_owned()
                    .await
                    .map_err(|_| "fixture generation release closed".to_owned())?
                    .forget();
                Err("fixture stops before source publication".to_owned())
            })
        }
    }));
    let generation_admission = server
        .workspace_generation_admission()
        .expect("configured generation admission");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let mut requests = tokio::task::JoinSet::new();
    let mut workspace_projects = Vec::with_capacity(WORKSPACE_COUNT);
    for workspace_index in 0..WORKSPACE_COUNT {
        let project_root = project_fixture
            .path()
            .join(format!("project-{workspace_index}"));
        tokio::fs::create_dir_all(&project_root)
            .await
            .expect("create admission project");
        gix::init(&project_root).expect("initialize independent admission workspace with Gix");
        record_admission_fixture_candidate(&project_root);
        let workspace_identity =
            agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
                .expect("resolve independent admission workspace")
                .workspace
                .workspace_id
                .to_string();
        workspace_projects.push((workspace_identity.clone(), project_root.clone()));
        let mutation_id = format!("mutation-workspace-{workspace_index}");
        for _ in 0..SESSION_COUNT_PER_WORKSPACE {
            let session = Arc::new(
                agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
                    &endpoint,
                    workspace_identity.clone(),
                    project_root.clone(),
                ),
            );
            for _ in 0..CALL_COUNT_PER_SESSION {
                let session = Arc::clone(&session);
                let changed_path = project_root
                    .join("src/lib.rs")
                    .to_string_lossy()
                    .into_owned();
                let mutation_id = mutation_id.clone();
                requests.spawn(async move {
                    let started = tokio::time::Instant::now();
                    (
                        session
                            .submit_runtime_generation_mutation(mutation_id, vec![changed_path])
                            .await,
                        started.elapsed(),
                    )
                });
            }
        }
    }

    let request_count = WORKSPACE_COUNT * SESSION_COUNT_PER_WORKSPACE * CALL_COUNT_PER_SESSION;
    let mut queued_by_workspace = std::collections::BTreeMap::<String, usize>::new();
    let mut latencies = Vec::with_capacity(request_count);
    while let Some(result) = requests.join_next().await {
        let (receipt, latency) = result.expect("join multi-session Runtime submission");
        let receipt = receipt.expect("multi-session Runtime submission receipt");
        receipt
            .validate()
            .expect("valid mutation admission receipt");
        if matches!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
        ) {
            *queued_by_workspace
                .entry(receipt.workspace_identity.clone())
                .or_default() += 1;
        }
        latencies.push(latency);
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "runtime-server-multi-workspace-session-admission workspaceCount={WORKSPACE_COUNT} sessionCount={} requestCount={request_count} p99Micros={}",
        WORKSPACE_COUNT * SESSION_COUNT_PER_WORKSPACE,
        p99.as_micros()
    );

    tokio::time::timeout(Duration::from_secs(1), async {
        while source_build_count.load(std::sync::atomic::Ordering::Relaxed) < WORKSPACE_COUNT {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("background workspace submissions must reach the Runtime Server");

    source_build_release.add_permits(WORKSPACE_COUNT);
    let build_start = tokio::time::Instant::now();
    let mut terminal_receipts = Vec::with_capacity(WORKSPACE_COUNT);
    for (workspace_identity, project_root) in &workspace_projects {
        terminal_receipts.push(
            tokio::time::timeout(
                Duration::from_secs(5),
                generation_admission.wait_terminal(workspace_identity, project_root),
            )
            .await
            .expect("admitted workspace generation task must reach a terminal state")
            .expect("read workspace generation terminal receipt"),
        );
    }
    eprintln!(
        "runtime-server-multi-workspace-build-start micros={} terminalReceipts={terminal_receipts:?}",
        build_start.elapsed().as_micros(),
    );
    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    assert_eq!(queued_by_workspace.len(), WORKSPACE_COUNT);
    assert!(
        queued_by_workspace.values().all(|queued| *queued == 1),
        "each workspace must enqueue exactly one mutation flight: {queued_by_workspace:?}"
    );
    assert!(
        p99 < Duration::from_millis(10),
        "multi-workspace Runtime submission IPC p99 exceeded 10ms: {p99:?}"
    );
    assert_eq!(
        source_build_count.load(std::sync::atomic::Ordering::Relaxed),
        WORKSPACE_COUNT
    );
}

#[path = "runtime_server_control/shared_admission.rs"]
mod shared_admission;
