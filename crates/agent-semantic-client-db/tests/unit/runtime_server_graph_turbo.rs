use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{
    GraphTurboEvaluationBuilder, GraphTurboResidentStatusHandle, RuntimeServer, RuntimeServerExit,
};
use agent_semantic_client_db::runtime_server_control::{
    GraphTurboResidentState, GraphTurboResidentStatus, RuntimeServerOperation, call_runtime_server,
};
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession;

use super::runtime_server_control::fixture_endpoint;

#[tokio::test(flavor = "multi_thread")]
async fn runtime_status_reports_daemon_owned_graph_turbo_resident() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let graph_turbo = GraphTurboResidentStatusHandle::new(GraphTurboResidentStatus {
        state: GraphTurboResidentState::Healthy,
        process_id: Some(4242),
        runtime_artifact: Some("/opt/asp/graph-turbo/bin/python3".to_owned()),
        execution_command_digest: Some(format!("blake3-256:{}", "a".repeat(64))),
        reason: None,
    });
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server")
    .with_graph_turbo_resident_status(graph_turbo.clone());
    let server = tokio::spawn(server.serve());

    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Reconcile,
        endpoint.runtime_binary_identity.clone(),
        "graph-turbo-prewarm".to_owned(),
    )
    .await
    .expect("prewarm Graph Turbo resident status");

    let status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "graph-turbo-status".to_owned(),
    )
    .await
    .expect("call runtime server status");
    let resident = status
        .graph_turbo_resident
        .expect("Graph Turbo resident status");
    assert_eq!(resident.process_id, Some(4242));
    assert_eq!(resident.state, GraphTurboResidentState::Healthy);

    graph_turbo.mutate(|status| status.state = GraphTurboResidentState::Failed);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    let failed = loop {
        let receipt = call_runtime_server(
            &endpoint,
            RuntimeServerOperation::Status,
            endpoint.runtime_binary_identity.clone(),
            "graph-turbo-failed-status".to_owned(),
        )
        .await
        .expect("call failed Graph Turbo status");
        if receipt
            .graph_turbo_resident
            .as_ref()
            .is_some_and(|status| status.state == GraphTurboResidentState::Failed)
        {
            break receipt;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Graph Turbo status notification did not publish before the deadline"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert_eq!(
        failed
            .graph_turbo_resident
            .expect("failed Graph Turbo status")
            .state,
        GraphTurboResidentState::Failed
    );

    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            value: "next-runtime-digest".to_owned(),
            algorithm: "blake3-256".to_owned(),
        },
        "graph-turbo-status-restart".to_owned(),
    )
    .await
    .expect("restart runtime server");
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn graph_turbo_lease_miss_rejects_before_python_builder() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_root = runtime_dir.path().join("graph-turbo-miss-workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create Graph Turbo project root");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 33).await;
    let invocation_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let builder_count = Arc::clone(&invocation_count);
    let builder: GraphTurboEvaluationBuilder =
        Arc::new(move |_workspace_identity, _project_root, _message| {
            let invocation_count = Arc::clone(&builder_count);
            Box::pin(async move {
                invocation_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(serde_json::json!({
                    "status": "rank-completed",
                    "authority": "candidate",
                    "result": {}
                }))
            })
        });
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_graph_turbo_evaluation_builder(builder);
    let mut workspace_count = server.workspace_registry().subscribe_workspace_count();
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());
    let session = WorkspaceDbIpcSession::for_runtime_server(
        &endpoint,
        "workspace-graph-turbo-miss".to_owned(),
        project_root,
    );

    let error = session
        .evaluate_graph_turbo(serde_json::json!({
            "snapshotDigest": "stale-snapshot",
            "workspaceGenerationRootDigest": "stale-generation"
        }))
        .await
        .expect_err("Graph Turbo request without an active generation must fail closed");
    assert!(
        error.contains("runtime-server-graph-turbo-continuation-stale"),
        "unexpected Graph Turbo lease-miss error: {error}"
    );
    assert_eq!(
        invocation_count.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "Python Graph Turbo builder must not run before Rust validates the active generation"
    );
    assert_eq!(
        *workspace_count.borrow_and_update(),
        0,
        "a projection-only v1 IPC read must not start a full workspace writer resident"
    );

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}
