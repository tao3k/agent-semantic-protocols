use std::path::Path;

use super::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority;

impl RuntimeServerWorkspaceRegistry {
    pub fn read_search_generation_authority(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<Option<WorkspaceSearchGenerationAuthority>, String> {
        let lease = match self.lease(workspace_identity, project_root) {
            Ok(lease) => lease,
            Err(_) => return Ok(None),
        };
        let authority = WorkspaceSearchGenerationAuthority::from_lease(&lease);
        authority.validate_binding(workspace_identity, &project_root.display().to_string())?;
        Ok(Some(authority))
    }
}
