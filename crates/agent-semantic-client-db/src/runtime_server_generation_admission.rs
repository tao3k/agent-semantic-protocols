// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Composition surface for Runtime Server workspace generation admission.

use crate::runtime_server::RuntimeServer;

impl RuntimeServer {
    pub fn with_workspace_generation_builder(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, None, None)
    }

    pub fn with_workspace_generation_builder_and_catalog(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, Some(catalog), None)
    }

    pub fn with_workspace_generation_builder_catalog_and_runtime_bundle_probe(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
        runtime_bundle_digest_probe: std::sync::Arc<
            dyn Fn() -> Result<Option<String>, String> + Send + Sync + 'static,
        >,
    ) -> Self {
        self.configure_workspace_generation_builder(
            source_builder,
            Some(catalog),
            Some(runtime_bundle_digest_probe),
        )
    }

    /// Restore only canonical generations already committed to workspace durability.
    pub fn with_workspace_generation_catalog(
        self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(None, Some(catalog), None)
    }
}
