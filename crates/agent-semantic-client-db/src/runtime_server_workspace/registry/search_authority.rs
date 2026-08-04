//! Compact search authority admission for a resident workspace generation.

use std::path::Path;

use super::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::{
    WorkspaceSearchGenerationAuthority, WorkspaceSearchGenerationAuthorityOpenReceipt,
    workspace_generation_pointer_path,
};

impl RuntimeServerWorkspaceRegistry {
    pub fn search_generation_authority_open_receipt(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceSearchGenerationAuthorityOpenReceipt, String> {
        let lease = self.lease(workspace_identity, project_root)?;
        let authority = WorkspaceSearchGenerationAuthority::from_lease(&lease);
        let pointer_path =
            workspace_generation_pointer_path(&self.root, workspace_identity, project_root)?;
        WorkspaceSearchGenerationAuthorityOpenReceipt::new(authority, &pointer_path)
    }
}
