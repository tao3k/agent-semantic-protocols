// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::core::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::{WorkspaceRuntimeOwnerRead, WorkspaceRuntimeSelectorRead};

impl RuntimeServerWorkspaceRegistry {
    pub fn read_runtime_selector(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        projection_kind: super::super::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let lease = match self.lease(workspace_identity, project_root) {
            Ok(lease) => lease,
            Err(_) => return Ok(WorkspaceRuntimeSelectorRead::GenerationMissing),
        };
        lease.read_runtime_selector(projection_kind, structural_selector)
    }

    pub fn read_runtime_owner(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        owner_path: &str,
    ) -> Result<WorkspaceRuntimeOwnerRead, String> {
        let lease = match self.lease(workspace_identity, project_root) {
            Ok(lease) => lease,
            Err(_) => return Ok(WorkspaceRuntimeOwnerRead::GenerationMissing),
        };
        let root_digest = lease.generation().source_snapshot.root_digest.clone();
        Ok(match lease.runtime_owner_snapshot(owner_path) {
            Some((generation_digest, owner)) => WorkspaceRuntimeOwnerRead::Owner {
                generation_digest,
                root_digest,
                owner,
            },
            None => WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest: lease.runtime_generation_digest().to_owned(),
                root_digest,
            },
        })
    }
}
