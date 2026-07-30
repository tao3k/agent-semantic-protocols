use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, RuntimeServerOperation, RuntimeServerState,
    call_runtime_server, prepare_runtime_server_endpoint_in,
};
use agent_semantic_client_db::WorkspaceDbRegistry;

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
    let server = RuntimeServer::bind(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

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
    let server = RuntimeServer::bind(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

    let cold_latencies =
        concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-expand").await;
    let p99_index = request_count.saturating_mul(99).div_ceil(100) - 1;
    let cold_p99 = cold_latencies[p99_index];
    assert!(
        cold_p99 < Duration::from_millis(10),
        "{request_count} concurrent adaptive expansion requests exceeded 10ms p99: {cold_p99:?}"
    );

    let latencies =
        concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-warm").await;
    let p99 = latencies[p99_index];
    assert!(
        p99 < Duration::from_millis(1),
        "{request_count} concurrent warm runtime requests exceeded sub-millisecond p99: {p99:?}"
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
}
