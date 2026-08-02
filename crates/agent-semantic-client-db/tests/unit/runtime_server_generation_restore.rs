use std::sync::Arc;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{
    RuntimeServer, RuntimeServerEvent, RuntimeServerExit,
};
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission_catalog::{
    RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerOperation, RuntimeServerState, call_runtime_server,
    prepare_runtime_server_endpoint_in,
};

async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> agent_semantic_client_db::RuntimeServerEndpoint {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    let catalog = agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
        &state_home,
    )
    .await
    .expect("load runtime artifact catalog");
    prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        catalog.mode_label(),
        &catalog.digest(),
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint")
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_is_healthy_while_workspace_restore_isolates_scope_failure() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let project_root = runtime_dir.path().join("stale-workspace");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create catalog project root");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(
        runtime_dir.path().join("workspace-admissions.v1.json"),
    )
    .await
    .expect("load admission catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-stale".to_owned(),
            project_root,
        })
        .await
        .expect("record stale scope");
    let endpoint = fixture_endpoint(&runtime_dir, 31).await;
    let (events, mut event_receipts) = tokio::sync::mpsc::unbounded_channel();
    let restore_started = Arc::new(tokio::sync::Notify::new());
    let release_restore = Arc::new(tokio::sync::Notify::new());
    let builder_started = Arc::clone(&restore_started);
    let builder_release = Arc::clone(&release_restore);
    let admission = WorkspaceGenerationAdmission::new(Arc::new(move |_, _, _| {
        let restore_started = Arc::clone(&builder_started);
        let release_restore = Arc::clone(&builder_release);
        Box::pin(async move {
            restore_started.notify_one();
            release_restore.notified().await;
            Err("fixture canonical generation missing".to_owned())
        })
    }))
    .with_catalog(catalog);
    let server = RuntimeServer::bind(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(
            &runtime_dir.path().join("state"),
        )),
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_admission(Arc::new(admission))
    .with_event_sender(events);
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let healthy_during_startup_restore = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "status-during-catalog-restore".to_owned(),
    )
    .await
    .expect("read Healthy daemon status during workspace restore");
    assert_eq!(
        healthy_during_startup_restore.state,
        RuntimeServerState::Healthy
    );

    restore_started.notified().await;
    let restoring = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "status-while-catalog-restore-is-building".to_owned(),
    )
    .await
    .expect("read Healthy daemon status while catalog restoration is building");
    assert_eq!(restoring.state, RuntimeServerState::Healthy);
    release_restore.notify_one();

    let event = event_receipts.recv().await.expect("scope failure event");
    let RuntimeServerEvent::WorkspaceGenerationRestoreFailed {
        workspace_identity,
        error,
    } = event
    else {
        panic!("expected workspace restore failure event: {event:?}");
    };
    assert_eq!(workspace_identity, "workspace-stale");
    assert!(!error.is_empty());
    let healthy = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        "status-after-scope-isolation".to_owned(),
    )
    .await
    .expect("read Healthy status after isolated scope failure");
    assert_eq!(healthy.state, RuntimeServerState::Healthy);

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}
