// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable Runtime query-generation value opened from resident Search authority.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

use super::query_generation_calibration::RuntimeSearchGenerationBuildResourceReceipt;

/// Immutable Runtime handle for one fully admitted Search generation.
pub struct RuntimeQueryGeneration {
    pub(super) generation_digest: String,
    pub(super) generation_token: AtomicU64,
    pub(super) resident: Option<Arc<RuntimeResidentReadClient>>,
    pub(super) execution_publication: Option<
        Arc<
            agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        >,
    >,
    pub(super) project_topology_attachment:
        Option<Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>>,
    pub(super) build_resource_receipt:
        std::sync::OnceLock<RuntimeSearchGenerationBuildResourceReceipt>,
}

impl RuntimeQueryGeneration {
    /// Opens the resident Search authority and derives its fusion capabilities.
    pub async fn open(
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
    ) -> Result<Self, String> {
        let resident = RuntimeResidentReadClient::open(pointer_path, project_root).await?;
        let generation_digest = resident.generation_digest();
        agent_semantic_search::ResidentSearchFusionCapabilities::from_open_generation(
            &resident
                .search_generation_authority()
                .content_search_generation,
        )?;
        Ok(Self {
            generation_digest,
            generation_token: AtomicU64::new(0),
            resident: Some(Arc::new(resident)),
            execution_publication: None,
            project_topology_attachment: None,
            build_resource_receipt: std::sync::OnceLock::new(),
        })
    }

    pub fn from_resident(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    ) -> Result<Self, String> {
        let generation_digest = resident.generation_digest();
        agent_semantic_search::ResidentSearchFusionCapabilities::from_open_generation(
            &resident
                .search_generation_authority()
                .content_search_generation,
        )?;
        Ok(Self {
            generation_digest,
            generation_token: AtomicU64::new(0),
            resident: Some(Arc::new(resident)),
            execution_publication: None,
            project_topology_attachment: None,
            build_resource_receipt: std::sync::OnceLock::new(),
        })
    }

    pub fn from_resident_with_execution_publication(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        execution_publication: agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
    ) -> Result<Self, String> {
        let generation = Self::from_resident(resident)?;
        execution_publication.validate().map_err(|error| {
            format!("validate resident Runtime execution publication: {error:?}")
        })?;
        if execution_publication.generation_digest.as_str() != generation.generation_digest
            || execution_publication.source_root_digest.as_str()
                != generation.resident().source_root_digest()
        {
            return Err(
                "reasonKind=runtime-query-generation-execution-publication-mismatch".to_owned(),
            );
        }
        Ok(Self {
            execution_publication: Some(Arc::new(execution_publication)),
            ..generation
        })
    }

    /// Atomically joins the already admitted Runtime generation and Project
    /// Topology product. Lower-level resident reads can exist without this
    /// attachment; Search Playbook cannot.
    pub fn with_project_topology_attachment(
        self,
        attachment: agent_semantic_topology::RuntimeProjectTopologyAttachment,
    ) -> Result<Self, String> {
        let execution_publication = self.execution_publication.as_deref().ok_or_else(|| {
            "reasonKind=runtime-project-topology-execution-publication-missing".to_owned()
        })?;
        if attachment.runtime_generation_digest() != self.generation_digest
            || attachment.runtime_execution_binding()
                != &execution_publication.runtime_execution_binding
        {
            return Err("reasonKind=runtime-project-topology-attachment-mismatch".to_owned());
        }
        Ok(Self {
            project_topology_attachment: Some(Arc::new(attachment)),
            ..self
        })
    }

    /// Returns the exact resident-generation digest.
    pub fn generation_digest(&self) -> &str {
        &self.generation_digest
    }

    /// Returns the content generation consumed by all Search accelerators.
    pub fn content_generation_digest(&self) -> &str {
        &self
            .resident()
            .search_generation_authority()
            .content_search_generation
            .content_generation_digest
    }

    /// Returns the process-local monotonic observation token.
    pub fn generation_token(&self) -> u64 {
        self.generation_token.load(Ordering::Acquire)
    }

    /// Returns the measured build-resource receipt when calibration completed.
    pub fn build_resource_receipt(&self) -> Option<&RuntimeSearchGenerationBuildResourceReceipt> {
        self.build_resource_receipt.get()
    }

    /// Borrows the resident read authority.
    pub fn resident(&self) -> &RuntimeResidentReadClient {
        self.resident
            .as_deref()
            .expect("ready query generation always owns a resident read client")
    }

    /// Borrows the immutable source/Runtime product installed with this generation.
    pub fn execution_publication(
        &self,
    ) -> Option<&agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication>
    {
        self.execution_publication.as_deref()
    }

    /// Borrows the exact topology authority required by Search Playbook.
    pub fn require_search_playbook_topology_attachment(
        &self,
    ) -> Result<&agent_semantic_topology::RuntimeProjectTopologyAttachment, String> {
        self.project_topology_attachment
            .as_deref()
            .ok_or_else(|| "reasonKind=runtime-project-topology-attachment-missing".to_owned())
    }

    pub fn native_syntax_state(&self) -> &'static str {
        // Parser facts are part of the immutable resident generation. There is
        // no second provider-owned native-syntax publication authority.
        "ready"
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        self.resident()
            .parser_owned_callable_selector_pairs(owner_paths)
    }

    /// Reads an exact selector from the parser-owned attachment without any
    /// filesystem, database, socket, or provider operation.
    pub fn read_runtime_selector(
        &self,
        projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        String,
    > {
        self.resident()
            .read_runtime_selector(projection_kind, structural_selector)
    }

    pub fn native_syntax_playbook_projection(
        &self,
        owner_paths: &[String],
    ) -> Result<
        (
            Vec<agent_semantic_search::NativeSyntaxProjection>,
            Vec<agent_semantic_search::NativeSyntaxRelation>,
            Vec<agent_semantic_search::NativeSyntaxDiagnostic>,
        ),
        String,
    > {
        self.resident()
            .native_syntax_playbook_projection(owner_paths)
    }

    /// Returns the capabilities admitted from this exact generation.
    pub fn fusion_capabilities(&self) -> agent_semantic_search::ResidentSearchFusionCapabilities {
        agent_semantic_search::ResidentSearchFusionCapabilities {
            lexical: self.resident().lexical_accelerator_is_ready(),
            resident_graph: self.resident().graph_generation_is_ready(),
            python_graph: false,
            byte_evidence: true,
            complete_byte_coverage: true,
        }
    }
}
