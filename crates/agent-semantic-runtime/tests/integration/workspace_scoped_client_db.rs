// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

// Workspace-owned client database layout contract.

use agent_semantic_artifacts::ProjectBinding;
use agent_semantic_artifacts::StateHomeLayout;

#[test]
fn workspace_client_database_path_is_unique_per_workspace() {
    let state_home = Path::new("/state-root");

    let binding_a = ProjectBinding::resolve(None, "repo-identity", "/checkout/worktree-a")
        .expect("workspace A binding");
    let binding_b = ProjectBinding::resolve(None, "repo-identity", "/checkout/worktree-b")
        .expect("workspace B binding");
    let layout = StateHomeLayout::new(state_home);
    let workspace_a = layout
        .workspace(&binding_a.workspace)
        .expect("workspace A paths");
    let workspace_b = layout
        .workspace(&binding_b.workspace)
        .expect("workspace B paths");

    assert!(workspace_a.root.starts_with(state_home.join("workspaces")));
    assert_eq!(workspace_a.facts, workspace_a.root.join("facts.turso"));
    assert_ne!(workspace_a.root, workspace_b.root);
    assert_ne!(workspace_a.facts, workspace_b.facts);
    assert_ne!(workspace_a.artifacts, workspace_b.artifacts);
    assert_ne!(workspace_a.observations, workspace_b.observations);
    assert!(
        !workspace_a
            .root
            .to_string_lossy()
            .contains("projects/by-id")
    );
}
