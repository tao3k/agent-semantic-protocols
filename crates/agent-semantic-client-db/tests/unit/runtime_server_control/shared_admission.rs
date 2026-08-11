//! Shared Runtime Server admission-plane concurrency scenarios.

use std::sync::Arc;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerOperation, RuntimeServerState, call_runtime_server,
};

use super::fixture_endpoint;

#[tokio::test(flavor = "multi_thread")]
async fn shared_runtime_admission_plane_is_workspace_keyed_and_drains() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let server = tokio::spawn(server.serve());
    let first = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            "workspace-first",
            runtime_dir.path().join("workspace-first"),
        ),
    );
    let second = Arc::new(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            "workspace-second",
            runtime_dir.path().join("workspace-second"),
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
    let p75 = samples[(samples.len() * 75).div_ceil(100) - 1];
    let max = *samples.last().expect("workspace health samples");
    assert!(
        p75 < 50_000,
        "shared Runtime workspace health p75 exceeded 50ms: p75Micros={p75}"
    );
    assert!(
        max < 500_000,
        "shared Runtime workspace health exceeded the 500ms hard boundary: maxMicros={max}"
    );
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
        "runtime-server-admission-concurrency requestCount={request_count} p75Micros={p75} maxMicros={max} workspaceCount=2"
    );
}
