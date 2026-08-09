use std::{path::Path, sync::Arc};

use super::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceExactProjectionDataPlaneClient, WorkspaceRuntimeOwnerRead,
    WorkspaceRuntimeSelectorRead, WorkspaceSearchGenerationAuthority,
    WorkspaceSearchGenerationDataPlaneClient, workspace_generation_pointer_path,
};

impl RuntimeServerWorkspaceRegistry {
    #[must_use]
    pub fn search_projection_slot_count(&self) -> usize {
        self.search_projection_slots.len()
    }

    fn projection_pointer_path(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<std::path::PathBuf, String> {
        if workspace_identity.trim().is_empty() {
            return Err("workspace identity must be non-empty text".to_owned());
        }
        if !project_root.is_absolute() {
            return Err(format!(
                "runtime workspace project root must be absolute: {}",
                project_root.display()
            ));
        }
        workspace_generation_pointer_path(&self.root, workspace_identity, project_root)
    }

    async fn search_projection_client(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<Arc<WorkspaceSearchGenerationDataPlaneClient>, String> {
        let pointer_path = self.projection_pointer_path(workspace_identity, project_root)?;
        let pointer =
            crate::runtime_server_workspace::WorkspaceGenerationPointerReader::open(&pointer_path)
                .await?;
        let snapshot = pointer.read()?;
        let key = (
            workspace_identity.to_owned(),
            project_root.to_string_lossy().into_owned(),
        );
        let slot = self
            .search_projection_slots
            .entry(key)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
            .clone();
        let mut current = slot.lock().await;
        if let Some((epoch, client)) = current.as_ref()
            && *epoch == snapshot.active_epoch
        {
            return Ok(Arc::clone(client));
        }
        let client = Arc::new(
            WorkspaceSearchGenerationDataPlaneClient::open(&pointer_path, project_root).await?,
        );
        *current = Some((client.authority().active_epoch, Arc::clone(&client)));
        Ok(client)
    }

    pub async fn read_projection_selector(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        projection_kind: ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let pointer_path = self.projection_pointer_path(workspace_identity, project_root)?;
        WorkspaceExactProjectionDataPlaneClient::open(&pointer_path)
            .await?
            .read_runtime_selector(projection_kind, structural_selector)
    }

    pub async fn read_projection_owner(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
    ) -> Result<WorkspaceRuntimeOwnerRead, String> {
        self.search_projection_client(workspace_identity, project_root)
            .await?
            .read_owner(owner_path)
    }

    pub async fn projection_search_generation_authority(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceSearchGenerationAuthority, String> {
        Ok(self
            .search_projection_client(workspace_identity, project_root)
            .await?
            .authority()
            .clone())
    }

    pub async fn read_projection_source_index(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        query: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        self.search_projection_client(workspace_identity, project_root)
            .await?
            .read_source_index(query, language_id, limit)
    }

    pub async fn read_projection_graph_facts(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        sources: &[crate::workspace_db_ipc::RuntimeGraphFactSource],
    ) -> Result<crate::workspace_db_ipc::RuntimeGraphFactsRead, String> {
        self.search_projection_client(workspace_identity, project_root)
            .await?
            .read_graph_facts(sources)
    }
}
