//! Runtime Server endpoint and workspace-store authority binding tests.

use std::sync::Arc;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_with_workspace_store;

#[tokio::test]
async fn endpoint_publishes_the_bound_workspace_store_authority() {
    let runtime_dir = tempfile::tempdir().expect("create endpoint authority fixture");
    let state_home = runtime_dir.path().join("state-home");
    std::fs::create_dir_all(&state_home).expect("create State Home fixture");
    let workspace_store = runtime_dir.path().join("persistent").join("workspaces");
    let endpoint = prepare_runtime_server_endpoint_with_workspace_store(
        &state_home,
        &workspace_store,
        std::path::Path::new("/runtime/asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"runtime-digest",
        ),
        "dev",
        "catalog-digest",
        7,
        "binding-7",
    )
    .await
    .expect("prepare endpoint with explicit store authority");

    assert_eq!(
        std::path::Path::new(&endpoint.workspace_store_path),
        workspace_store
    );
}

#[tokio::test]
async fn server_rejects_a_split_endpoint_and_workspace_store_authority() {
    let runtime_dir = tempfile::tempdir().expect("create split authority fixture");
    let (endpoint, artifact_catalog) =
        super::runtime_server_control::fixture_endpoint(&runtime_dir, 8).await;
    let split_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store_at_root(
            runtime_dir.path().join("different-workspaces"),
        )
        .await
        .expect("prepare split workspace store");

    let error = match RuntimeServer::bind_with_artifact_catalog(
        endpoint,
        Arc::new(WorkspaceDbRegistry::default()),
        split_store,
        artifact_catalog,
    )
    .await
    {
        Ok(_) => panic!("split endpoint/store authority must fail closed"),
        Err(error) => error,
    };
    assert!(error.contains("endpoint workspace store authority mismatch"));
}
