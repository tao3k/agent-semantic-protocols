use std::{path::Path, sync::Arc};

use super::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceExactProjectionDataPlaneClient, WorkspaceRuntimeOwnerRead,
    WorkspaceRuntimeSelectorRead, WorkspaceSearchGenerationAuthority,
    WorkspaceSearchGenerationDataPlaneClient,
};

impl RuntimeServerWorkspaceRegistry {
    fn resident_search_projection_client(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<Arc<WorkspaceSearchGenerationDataPlaneClient>, String> {
        let entry = self
            .ready_entry(workspace_identity, project_root)?
            .ok_or_else(|| {
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned()
            })?;
        entry
            .publisher
            .search_generation_data_plane()
            .ok_or_else(|| {
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned()
            })
    }

    fn resident_exact_projection_client(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<Arc<WorkspaceExactProjectionDataPlaneClient>, String> {
        let entry = self
            .ready_entry(workspace_identity, project_root)?
            .ok_or_else(|| {
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned()
            })?;
        entry
            .publisher
            .search_generation_exact_projection()
            .ok_or_else(|| {
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned()
            })
    }

    pub async fn prepare_search_projection_client(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<(), String> {
        let entry = self.entry(workspace_identity, project_root).await?;
        entry
            .publisher
            .search_generation_data_plane()
            .ok_or_else(|| {
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned()
            })
            .map(|_| ())
    }

    pub async fn read_projection_selector(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        projection_kind: ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        self.resident_exact_projection_client(workspace_identity, project_root)?
            .read_runtime_selector(projection_kind, structural_selector)
    }

    pub async fn read_projection_owner(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
    ) -> Result<WorkspaceRuntimeOwnerRead, String> {
        self.resident_search_projection_client(workspace_identity, project_root)?
            .read_owner(owner_path)
    }

    pub async fn read_projection_merkle_owner(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead, String> {
        self.resident_search_projection_client(workspace_identity, project_root)?
            .read_merkle_owner(owner_path)
    }

    pub async fn projection_search_generation_authority(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceSearchGenerationAuthority, String> {
        self.resident_search_projection_client(workspace_identity, project_root)
            .map(|client| client.authority().clone())
    }

    pub async fn read_projection_source_index(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        query: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        self.resident_search_projection_client(workspace_identity, project_root)?
            .read_source_index(query, language_id, limit)
    }

    pub async fn read_projection_graph_facts(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        sources: &[crate::workspace_db_ipc::RuntimeGraphFactSource],
    ) -> Result<crate::workspace_db_ipc::RuntimeGraphFactsRead, String> {
        self.resident_search_projection_client(workspace_identity, project_root)?
            .read_graph_facts(sources)
    }
}
