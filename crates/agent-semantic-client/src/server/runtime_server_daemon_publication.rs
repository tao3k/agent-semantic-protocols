// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Adapts resident-view publications and identity handoff terminals for the daemon.

pub(super) fn resident_workspace_view_publication(
    workspace_store_root: &std::path::Path,
    view: agent_semantic_client_db::runtime_server_publication::ResidentWorkspaceViewPublished,
) -> Result<
    agent_semantic_client_db::runtime_server_publication::WorkspaceGenerationPublished,
    String,
> {
    let resolved = agent_semantic_runtime::state_core::ResolvedState::resolve(&view.project_root)?;
    let project_id =
        agent_semantic_client_protocol::ClientProjectId::new(resolved.repo.repo_id.to_string())?;
    let workspace_id = agent_semantic_client_protocol::ClientWorkspaceIdentity::new(
        view.workspace_identity.clone(),
    )?;
    let resident_pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            workspace_store_root,
            &view.workspace_identity,
            &view.project_root,
        )?;
    Ok(
        agent_semantic_client_db::runtime_server_publication::WorkspaceGenerationPublished {
            project_id,
            workspace_id,
            project_root: view.project_root,
            resident_pointer_path,
            generation_digest: view.generation_digest,
            resident_view_digest: Some(view.resident_view_digest),
        },
    )
}

pub(super) fn normalize_server_shutdown_for_identity_handoff(
    identity_handoff_requested: bool,
    server_result: Result<(), String>,
) -> Result<(), String> {
    if identity_handoff_requested {
        Ok(())
    } else {
        server_result
    }
}
