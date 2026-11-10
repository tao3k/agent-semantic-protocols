// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Constructs workspace IPC sessions from lifecycle-published locator bindings.

use super::protocol::{
    WorkspaceDbIpcSession, WorkspaceDbSessionBinding, WorkspaceDbSessionProfile,
};
use crate::runtime_server_hook_admission_locator::RuntimeHookAdmissionEndpointBinding;
use std::path::{Path, PathBuf};

impl WorkspaceDbIpcSession {
    pub(crate) fn for_runtime_locator(
        binding: &RuntimeHookAdmissionEndpointBinding,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
    ) -> Self {
        let workspace_identity = workspace_identity.into();
        let generation_pointer_path =
            crate::runtime_server_workspace::workspace_generation_pointer_path(
                Path::new(&binding.workspace_store_path),
                &workspace_identity,
                &project_root,
            )
            .ok();
        Self::from_binding_with_profile(
            WorkspaceDbSessionBinding {
                workspace_identity,
                project_root: Some(project_root),
                transport_contract_digest: binding.transport_contract_digest.clone(),
                owner_epoch: binding.owner_epoch,
                runtime_binary_path: binding.runtime_artifact_path.clone(),
                runtime_binary_digest: binding.runtime_artifact_digest.clone(),
                binding_token: binding.binding_token.clone(),
                data_endpoint: binding.data_endpoint.clone(),
                generation_pointer_path,
            },
            WorkspaceDbSessionProfile::HookReadOnly,
        )
    }
}
