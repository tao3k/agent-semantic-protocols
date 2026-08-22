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
        self.configure_workspace_generation_builder(source_builder, None, Some(catalog))
    }

    pub fn with_workspace_generation_and_owner_builders(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        owner_projection_builder: crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
    ) -> Self {
        self.configure_workspace_generation_builder(
            source_builder,
            Some(owner_projection_builder),
            None,
        )
    }

    pub fn with_workspace_generation_and_owner_builders_and_catalog(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        owner_projection_builder: crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(
            source_builder,
            Some(owner_projection_builder),
            Some(catalog),
        )
    }

    /// Restore only canonical generations already committed to workspace durability.
    pub fn with_workspace_generation_catalog(
        self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(None, None, Some(catalog))
    }
}
