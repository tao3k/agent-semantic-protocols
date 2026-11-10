// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lightweight process-cold routing proof for durable Search and Query readers.

use std::path::PathBuf;

use super::RuntimeServerWorkspaceRegistry;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DurableQueryBinding {
    pub(crate) generation_digest: String,
    pub(crate) runtime_bundle_digest: String,
}

impl RuntimeServerWorkspaceRegistry {
    pub(crate) fn publish_durable_query_binding(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        binding: DurableQueryBinding,
    ) {
        self.durable_query_bindings
            .write()
            .insert((workspace_identity, project_root), binding);
    }

    pub(crate) fn validate_durable_query_binding(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        generation_digest: &str,
        runtime_bundle_digest: &str,
    ) -> Result<(), String> {
        let bindings = self.durable_query_bindings.read();
        let binding = bindings
            .get(&(workspace_identity.to_owned(), project_root.to_path_buf()))
            .ok_or_else(|| {
                format!(
                    "durable query workspace is not admitted: workspaceIdentity={workspace_identity}"
                )
            })?;
        if binding.generation_digest != generation_digest
            || binding.runtime_bundle_digest != runtime_bundle_digest
        {
            return Err("durable query binding identity drift".to_owned());
        }
        Ok(())
    }
}
