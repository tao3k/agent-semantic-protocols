use std::sync::Arc;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
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
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    let catalog =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(
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

#[tokio::test(flavor = "multi_thread")]
async fn daemon_startup_does_not_eagerly_restore_registered_workspaces() {
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
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 31).await;
    let build_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let builder_count = Arc::clone(&build_count);
    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        move |_, _, _, _, _changed_paths, _provider_target, _cancellation| {
            builder_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move {
                Err(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage::DurableRestore,
                    "fixture canonical generation missing",
                ))
            })
        },
    ))
    .with_catalog(catalog);
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::with_state_home(
            &runtime_dir.path().join("state"),
        )),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server")
    .with_workspace_generation_admission(Arc::new(admission));
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let healthy = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "status-with-on-demand-workspace-restore".to_owned(),
    )
    .await
    .expect("read Healthy status before on-demand workspace admission");
    assert_eq!(healthy.state, RuntimeServerState::Healthy);
    assert_eq!(
        build_count.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "daemon startup must not restore or build catalog workspaces eagerly"
    );

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}
