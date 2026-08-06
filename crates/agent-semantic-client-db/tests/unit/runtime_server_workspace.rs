//! Runtime Server workspace generation tests split by responsibility.

#[path = "runtime_server_workspace/overlay_projection.rs"]
mod overlay_projection;

#[path = "runtime_server_workspace/durability.rs"]
mod durability;
#[path = "runtime_server_workspace/resident_source_index.rs"]
mod resident_source_index;
#[path = "runtime_server_workspace/transition_checkpoint.rs"]
mod transition_checkpoint;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_shutdown_cancels_each_resident_entry() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let registry =
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            temporary.path().to_path_buf(),
        )
        .expect("registry");
    let project_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&project_root).expect("project root");
    registry
        .prepare_resident_workspace_scope("workspace-cancellation", &project_root)
        .await
        .expect("resident workspace scope");
    let cancellation = registry
        .workspace_context("workspace-cancellation")
        .expect("workspace runtime context");
    assert!(!cancellation.is_cancelled());
    registry.shutdown().await.expect("registry shutdown");
    assert!(cancellation.is_cancelled());
}
