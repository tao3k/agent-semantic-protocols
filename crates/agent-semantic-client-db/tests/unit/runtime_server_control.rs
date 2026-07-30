use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, RuntimeServerOperation, RuntimeServerState,
    call_runtime_server, prepare_runtime_server_endpoint_in, prewarm_runtime_server_status_memory,
    runtime_server_status_memory_metrics,
};

async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> agent_semantic_client_db::RuntimeServerEndpoint {
    prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint")
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

#[tokio::test(flavor = "current_thread")]
async fn runtime_endpoint_omits_workspace_and_process_identity() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 21).await;
    let value = serde_json::to_value(&endpoint).expect("encode runtime server endpoint");
    let object = value.as_object().expect("endpoint JSON object");

    assert!(!object.contains_key("workspaceIdentity"));
    assert!(!object.contains_key("ownerPid"));
    assert!(endpoint.socket_path.len() <= 103);
    assert_eq!(
        agent_semantic_client_db::runtime_server_endpoint_path(runtime_dir.path()),
        runtime_dir
            .path()
            .join("runtime")
            .join("server")
            .join("endpoint.v1.json")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn restart_is_idempotent_for_the_expected_runtime_digest() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 22).await;
    let mut request = RuntimeServerControlRequest {
        schema_id: "agent.semantic-protocols.runtime-server-control-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Restart,
        expected_runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
        request_id: "restart-idempotency".to_owned(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
    };

    assert!(
        !request
            .requires_restart(&endpoint)
            .expect("validate already-current restart")
    );
    request.expected_runtime_artifact_digest = "next-runtime-digest".to_owned();
    assert!(
        request
            .requires_restart(&endpoint)
            .expect("validate stale-runtime restart")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn typed_status_and_restart_use_the_real_runtime_server() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 23).await;
    let server = RuntimeServer::bind(endpoint.clone(), Arc::new(WorkspaceDbRegistry::default()))
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
    let endpoint = fixture_endpoint(&runtime_dir, 29).await;
    let endpoint_path = runtime_dir.path().join("runtime-server-endpoint.v1.json");
    let server = RuntimeServer::bind_and_publish(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        &endpoint_path,
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
    let endpoint = fixture_endpoint(&runtime_dir, 31).await;
    let blocking_parent = runtime_dir.path().join("not-a-directory");
    tokio::fs::write(&blocking_parent, b"block directory creation")
        .await
        .expect("write blocking parent");
    let endpoint_path = blocking_parent.join("runtime-server-endpoint.v1.json");

    let error = RuntimeServer::bind_and_publish(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        &endpoint_path,
    )
    .await
    .err()
    .expect("endpoint publication must fail");
    assert!(error.contains("failed to create Runtime Server endpoint directory"));
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
    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(16)
        .clamp(32, 512);
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 24).await;
    let server = RuntimeServer::bind(endpoint.clone(), Arc::new(WorkspaceDbRegistry::default()))
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
    assert_eq!(cold_metrics.snapshot_decode_count, 1);

    let latencies = concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-warm").await;
    let p99 = latencies[p99_index];
    assert!(
        p99 < Duration::from_millis(1),
        "{request_count} concurrent warm runtime requests exceeded sub-millisecond p99: {p99:?}"
    );
    let warm_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("warm status memory metrics");
    assert_eq!(warm_metrics.reader_open_count, 1);
    assert_eq!(warm_metrics.snapshot_decode_count, 1);
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
    assert_eq!(draining_metrics.snapshot_decode_count, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn adaptive_persistent_control_lanes_are_millisecond_bounded_at_p99() {
    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(4)
        .clamp(16, 128);
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 26).await;
    let server = RuntimeServer::bind(endpoint.clone(), Arc::new(WorkspaceDbRegistry::default()))
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
    let p99 = latencies[request_count.saturating_mul(99).div_ceil(100) - 1];
    assert!(
        p99 < Duration::from_millis(10),
        "{request_count} persistent control requests exceeded 10ms at p99: {p99:?}"
    );
    eprintln!(
        "runtime-server-persistent-control requestCount={request_count} p99Micros={}",
        p99.as_micros()
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
    let endpoint = fixture_endpoint(&runtime_dir, 27).await;
    let server = RuntimeServer::bind(endpoint.clone(), Arc::new(WorkspaceDbRegistry::default()))
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
async fn shared_runtime_admission_plane_is_workspace_keyed_and_drains() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let endpoint = fixture_endpoint(&runtime_dir, 29).await;
    let server = RuntimeServer::bind(endpoint.clone(), Arc::new(WorkspaceDbRegistry::default()))
        .await
        .expect("bind Runtime Server");
    let server = tokio::spawn(server.serve());
    let first = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            "workspace-first",
        ),
    );
    let second = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            "workspace-second",
        ),
    );
    first.health().await.expect("prewarm first workspace lane");
    second
        .health()
        .await
        .expect("prewarm second workspace lane");

    let parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let request_count = parallelism.saturating_mul(32).max(64);
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..request_count {
        let session = if index % 2 == 0 {
            Arc::clone(&first)
        } else {
            Arc::clone(&second)
        };
        tasks.spawn(async move {
            let started = tokio::time::Instant::now();
            session.health().await.expect("warm workspace health");
            started.elapsed().as_micros()
        });
    }
    let mut samples = Vec::with_capacity(request_count);
    while let Some(sample) = tasks.join_next().await {
        samples.push(sample.expect("join warm workspace health"));
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() - 1) * 99 / 100];
    let shutdown_error = first
        .shutdown()
        .await
        .expect_err("workspace request must not stop the shared Runtime Server");
    assert!(shutdown_error.contains("runtime-server-shutdown-control-required"));

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        "next-runtime-digest".to_owned(),
        "restart-data-plane".to_owned(),
    )
    .await
    .expect("request Runtime Server restart");
    assert_eq!(restart.state, RuntimeServerState::Draining);
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
    eprintln!(
        "runtime-server-admission-concurrency requestCount={request_count} p99Micros={p99} workspaceCount=2"
    );
}
