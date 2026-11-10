// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::resolve_registered_workspace;

#[tokio::test]
async fn explicit_workspace_is_resolved_as_a_registered_identity_not_a_path() {
    let state_home = tempfile::tempdir().expect("isolated State Home");
    let current = agent_semantic_client_core::state_core::ResolvedState::resolve(
        std::env::current_dir().expect("current repository directory"),
    )
    .expect("current project identity");
    let workspace_id = current.workspace.workspace_id.to_string();
    let catalog_path = agent_semantic_artifacts::StateHomeLayout::new(state_home.path())
        .runtime_state()
        .serving()
        .workspace_admission_catalog();
    std::fs::create_dir_all(catalog_path.parent().expect("catalog parent"))
        .expect("catalog directory");
    let catalog = agent_semantic_client_db::runtime_server_admission_catalog::
        RuntimeWorkspaceAdmissionCatalog::load(catalog_path)
        .await
        .expect("empty admission catalog");
    catalog
        .record(
            agent_semantic_client_db::runtime_server_admission_catalog::
                RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                    workspace_id.clone(),
                    current.workspace.root.clone(),
                )
                .expect("current workspace admission"),
        )
        .await
        .expect("record current workspace");

    assert_eq!(
        resolve_registered_workspace(Some(&workspace_id), state_home.path())
            .await
            .expect("registered workspace root"),
        current.workspace.root
    );
}

#[tokio::test]
async fn filesystem_path_is_not_accepted_as_an_explicit_workspace_identity() {
    let state_home = tempfile::tempdir().expect("isolated State Home");
    let error = resolve_registered_workspace(Some("/tmp/not-a-workspace-id"), state_home.path())
        .await
        .expect_err("filesystem paths are not registered workspace identities");
    assert!(
        error.contains("workspace admission catalog has no binding"),
        "error={error}"
    );
}
