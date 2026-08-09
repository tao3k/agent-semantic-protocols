//! Opens immutable Runtime Server generation data planes for resident queries.

use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeServerSearchDataPlane {
    session: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    authority:
        agent_semantic_client_db::runtime_server_workspace::WorkspaceSearchGenerationAuthority,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeServerSearchSnapshot {
    pub(crate) source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub(crate) workspace_generation:
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
}

impl RuntimeServerSearchDataPlane {
    pub(crate) fn current_snapshot(&self) -> Result<RuntimeServerSearchSnapshot, String> {
        let authority = &self.authority;
        let workspace_generation =
            agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
                authority.workspace_generation.clone(),
            )
            .map_err(|error| format!("resident search generation evidence is incomplete: {error}"))?;
        Ok(RuntimeServerSearchSnapshot {
            source_snapshot: authority.source_snapshot.clone(),
            workspace_generation,
        })
    }

    pub(crate) fn project_resolutions(
        &self,
    ) -> &[agent_semantic_runtime::AdmittedProjectResolution] {
        &self.authority.project_resolutions
    }

    pub(crate) async fn read_source_index(
        &self,
        request: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbSourceIndexLookupRequest,
    ) -> Result<agent_semantic_client_db::ClientDbSourceIndexLookupResult, String> {
        self.session.read_source_index(&request).await
    }

    pub(crate) async fn read_owner(
        &self,
        owner_path: &str,
    ) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String>
    {
        self.session.read_runtime_owner(owner_path).await
    }
}

pub(crate) async fn runtime_server_search_data_plane_async(
    project_root: &Path,
) -> Result<RuntimeServerSearchDataPlane, String> {
    let session =
        crate::server::runtime_server::runtime_server_workspace_session_async(project_root).await?;
    let authority = session.runtime_search_generation_authority().await?;
    Ok(RuntimeServerSearchDataPlane { session, authority })
}

/// Reads one exact projection through the resident Runtime Server authority.
///
/// The CLI process must never open the generation pointer or mmap segment. The
/// daemon owns the load-once data plane, epoch replacement, telemetry, and
/// generation reconciliation for every workspace.
pub(crate) async fn runtime_server_workspace_exact_projection_async(
    project_root: &Path,
    language_id: agent_semantic_client_core::LanguageId,
    projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
    structural_selector: &str,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String>
{
    let session =
        crate::server::runtime_server::runtime_server_workspace_session_async(project_root).await?;
    session
        .read_runtime_selector(language_id, projection_kind, structural_selector)
        .await
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_generation_data_plane.rs"]
mod tests;
