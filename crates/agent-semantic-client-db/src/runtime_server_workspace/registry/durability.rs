// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use tokio::sync::watch;

use super::RuntimeServerWorkspaceRegistry;

#[cfg(test)]
#[path = "../../../tests/unit/runtime_server_workspace_durability.rs"]
mod runtime_server_workspace_durability_tests;

impl RuntimeServerWorkspaceRegistry {
    pub fn subscribe_generation_durability(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<
        watch::Receiver<
            Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
        >,
        String,
    > {
        let entry = self
            .ready_entry(workspace_identity, project_root)?
            .ok_or_else(|| {
                format!(
                    "runtime workspace is not checked out: workspaceIdentity={workspace_identity}"
                )
            })?;
        Ok(entry.durability.subscribe())
    }

    pub fn generation_durability(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>, String>
    {
        let Some(entry) = self.ready_entry(workspace_identity, project_root)? else {
            return Ok(None);
        };
        Ok(entry.durability.borrow().clone())
    }
}
