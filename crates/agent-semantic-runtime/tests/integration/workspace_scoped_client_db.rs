use std::path::Path;

// Workspace-owned client database layout contract.

use agent_semantic_runtime::state_core::{RepoId, StatePaths, WorkspaceId};

#[test]
fn workspace_client_database_path_is_unique_per_workspace() {
    let state_home = Path::new("/state-root");

    let project_id = RepoId("project-123".to_owned());
    let workspace_a_id = WorkspaceId("workspace-a".to_owned());
    let workspace_b_id = WorkspaceId("workspace-b".to_owned());

    let workspace_a = StatePaths::new(state_home, &project_id, &workspace_a_id);
    let workspace_b = StatePaths::new(state_home, &project_id, &workspace_b_id);

    let expected_workspace_a_client_dir = state_home
        .join("projects")
        .join("by-id")
        .join("project-123")
        .join("workspaces")
        .join("workspace-a")
        .join("live")
        .join("client");

    assert_eq!(workspace_a.client_dir, expected_workspace_a_client_dir);
    assert_eq!(
        workspace_a.client_db_path,
        expected_workspace_a_client_dir.join("facts.turso")
    );

    assert_ne!(workspace_a.workspace_dir, workspace_b.workspace_dir);
    assert_ne!(workspace_a.client_dir, workspace_b.client_dir);
    assert_ne!(workspace_a.client_db_path, workspace_b.client_db_path);
    assert_ne!(workspace_a.hooks_dir, workspace_b.hooks_dir);
    assert_ne!(workspace_a.artifacts_dir, workspace_b.artifacts_dir);
}
