// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::inspect_one_workspace_topology;
use agent_semantic_artifacts::MaterializedWorkspaceState;
use agent_semantic_artifacts::ProjectBinding;
use agent_semantic_artifacts::WorkspaceStatePaths;
use agent_semantic_artifacts::WorkspaceTopologyState;
use agent_semantic_runtime::state_core::ResolvedState;
use std::path::Path;
use std::path::PathBuf;

fn materialized(binding: ProjectBinding) -> MaterializedWorkspaceState {
    let root = PathBuf::from("/state/workspaces/test");
    MaterializedWorkspaceState {
        binding,
        paths: WorkspaceStatePaths {
            facts: root.join("facts.turso"),
            artifacts: root.join("artifacts"),
            observations: root.join("observations"),
            root,
        },
        last_observed_at_ms: 0,
        byte_count: 0,
    }
}

#[test]
fn current_gix_worktree_is_reachable_without_reading_generation_content() {
    let state_home = Path::new("/state-home-test");
    let state = ResolvedState::resolve_with_state_home(env!("CARGO_MANIFEST_DIR"), state_home)
        .expect("resolve current Gix worktree");
    let workspace = materialized(state.project_binding().expect("current project binding"));

    let observation = inspect_one_workspace_topology(&workspace, state_home);

    assert_eq!(observation.state, WorkspaceTopologyState::Reachable);
    assert_eq!(observation.reason, "gix-project-binding-exact");
}

#[test]
fn disappeared_worktree_is_classified_missing() {
    let root = std::env::temp_dir().join(format!("asp-missing-worktree-{}", std::process::id()));
    assert!(!root.exists(), "fixture root must remain absent");
    let binding = ProjectBinding::resolve_with_private_git_dir(
        None,
        "git-common-dir:/missing/repository",
        &root,
        Some(root.join(".git")),
    )
    .expect("missing worktree binding remains structurally valid");

    let observation =
        inspect_one_workspace_topology(&materialized(binding), Path::new("/state-home-test"));

    assert_eq!(observation.state, WorkspaceTopologyState::Missing);
    assert_eq!(observation.reason, "canonical-worktree-root-missing");
}

#[test]
fn path_rebound_to_another_git_identity_is_not_reachable() {
    let state_home = Path::new("/state-home-test");
    let current = ResolvedState::resolve_with_state_home(env!("CARGO_MANIFEST_DIR"), state_home)
        .expect("resolve current Gix worktree");
    let current_binding = current.project_binding().expect("current project binding");
    let stale = ProjectBinding::resolve_with_private_git_dir(
        None,
        "git-common-dir:/another/object-database",
        &current_binding.workspace.canonical_root,
        current_binding.workspace.private_git_dir.as_deref(),
    )
    .expect("stale binding");

    let observation = inspect_one_workspace_topology(&materialized(stale), state_home);

    assert_eq!(observation.state, WorkspaceTopologyState::IdentityMismatch);
    assert_eq!(observation.reason, "gix-project-binding-drift");
}
