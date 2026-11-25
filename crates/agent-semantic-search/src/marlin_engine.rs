// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Converged Marlin-native search engine facade wired exclusively through
//! two host-supplied ports:
//!
//! 1. [`OrgDocumentProjectionPort`] (implemented by Marlin `marlin-agent-context`)
//! 2. [`SearchEngineStoragePort`] (implemented by Marlin `marlin-agent-storage`)
//!
//! This facade embeds neither `orgize` FFI nor a second Turso database engine.

use std::path::Path;
use std::sync::Arc;

use agent_semantic_search_playbook::{
    ProgressiveSearchPlaybookError, ProgressiveSearchPlaybookRequest, SearchPlaybookPlanInput,
    admit_search_playbook_plan,
};
use agent_semantic_topology::{
    AgentOrgTopologyOverlay, AgentOrgTopologyOverlayError, AgentTopologyRelationship,
    OrgDocumentProjectionPort, ProjectTopologyManifest, ProjectTopologyManifestError,
    ProjectTopologySourceProgram, ProjectTopologySourceProgramError,
};

use crate::{
    ConvergedSearchExecutionError, ConvergedSearchExecutionReport, GraphOwnerRankReport,
    GraphOwnerRankRequest, SearchArtifactBlob, SearchArtifactHash, SearchEngineStoragePort,
    SearchGenerationDigest, SearchGenerationPointerRecord, SearchGenerationPointerSwapRequest,
    SearchPointerKey, SearchProjectId, SearchStoragePortError, SearchTopologyEdgeRecord,
    SearchWorkspaceId, SourceIndexRankReport, SourceIndexRankRequest, execute_admitted_playbook,
    rank_graph_owner_report, rank_source_index_report,
};

/// Default CAS pointer key for one workspace's active search generation.
pub const DEFAULT_SEARCH_GENERATION_POINTER_KEY: &str = "asp:search:active-generation";

/// Converged search engine entry point for Marlin embedding.
pub struct MarlinConvergedSearchEngine {
    org_port: Arc<dyn OrgDocumentProjectionPort>,
    storage_port: Arc<dyn SearchEngineStoragePort>,
}

impl MarlinConvergedSearchEngine {
    #[must_use]
    pub fn new(
        org_port: Arc<dyn OrgDocumentProjectionPort>,
        storage_port: Arc<dyn SearchEngineStoragePort>,
    ) -> Self {
        Self {
            org_port,
            storage_port,
        }
    }

    #[must_use]
    pub fn org_port(&self) -> &dyn OrgDocumentProjectionPort {
        self.org_port.as_ref()
    }

    #[must_use]
    pub fn storage_port(&self) -> &dyn SearchEngineStoragePort {
        self.storage_port.as_ref()
    }

    /// Admit a canonical Project Topology manifest through Marlin's Org port.
    pub fn admit_topology_manifest(
        &self,
        org_source: &str,
    ) -> Result<ProjectTopologyManifest, ProjectTopologyManifestError> {
        ProjectTopologyManifest::parse_with_port(org_source, self.org_port.as_ref())
    }

    /// Admit a canonical Project Topology source program through Marlin's Org port.
    pub fn admit_topology_program(
        &self,
        org_source: &str,
    ) -> Result<ProjectTopologySourceProgram, ProjectTopologySourceProgramError> {
        ProjectTopologySourceProgram::parse_with_port(org_source, self.org_port.as_ref())
    }

    /// Admit an Agent-authored Org topology overlay through Marlin's Org port.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors AgentOrgTopologyOverlay::admit_with_port explicit identity fields"
    )]
    pub fn admit_org_overlay(
        &self,
        source_generation_digest: impl Into<String>,
        base_topology_generation_digest: impl Into<String>,
        selector: impl Into<String>,
        evidence_digest: impl Into<String>,
        agent_identity_digest: impl Into<String>,
        prompt_digest: impl Into<String>,
        summary: impl Into<String>,
        org_source: impl Into<String>,
        relationships: Vec<AgentTopologyRelationship>,
    ) -> Result<AgentOrgTopologyOverlay, AgentOrgTopologyOverlayError> {
        AgentOrgTopologyOverlay::admit_with_port(
            source_generation_digest,
            base_topology_generation_digest,
            selector,
            evidence_digest,
            agent_identity_digest,
            prompt_digest,
            summary,
            org_source,
            relationships,
            self.org_port.as_ref(),
        )
    }

    /// Store a derived generation artifact and atomically advance the workspace pointer.
    pub fn publish_generation_artifact(
        &self,
        project_id: SearchProjectId,
        workspace_id: SearchWorkspaceId,
        expected_hash: Option<&SearchArtifactHash>,
        generation_digest: SearchGenerationDigest,
        media_type: impl Into<String>,
        body: Vec<u8>,
    ) -> Result<SearchGenerationPointerRecord, SearchStoragePortError> {
        let blob = SearchArtifactBlob::from_bytes(project_id.clone(), media_type, body)?;
        let artifact_hash = blob.artifact_hash.clone();
        self.storage_port.put_artifact(blob)?;
        let pointer_key = SearchPointerKey::new(DEFAULT_SEARCH_GENERATION_POINTER_KEY)?;
        self.storage_port
            .compare_and_swap_generation_pointer(SearchGenerationPointerSwapRequest {
                project_id,
                workspace_id,
                pointer_key,
                expected_hash: expected_hash.cloned(),
                new_generation_digest: generation_digest,
                new_artifact_hash: artifact_hash,
            })
    }

    /// Load the active generation pointer and its immutable artifact payload.
    pub fn load_active_generation_artifact(
        &self,
        project_id: &SearchProjectId,
        workspace_id: &SearchWorkspaceId,
    ) -> Result<Option<(SearchGenerationPointerRecord, SearchArtifactBlob)>, SearchStoragePortError>
    {
        let pointer_key = SearchPointerKey::new(DEFAULT_SEARCH_GENERATION_POINTER_KEY)?;
        let Some(pointer) =
            self.storage_port
                .get_generation_pointer(project_id, workspace_id, &pointer_key)?
        else {
            return Ok(None);
        };
        let blob = self
            .storage_port
            .get_artifact(project_id, &pointer.artifact_hash)?
            .ok_or_else(|| SearchStoragePortError::MissingArtifact {
                project_id: project_id.clone(),
                artifact_hash: pointer.artifact_hash.clone(),
            })?;
        Ok(Some((pointer, blob)))
    }

    /// Persist derived topology edges into Marlin's unified storage backend.
    pub fn persist_topology_edges(
        &self,
        edges: &[SearchTopologyEdgeRecord],
    ) -> Result<usize, SearchStoragePortError> {
        self.storage_port.append_topology_edges(edges)
    }

    /// Read persisted topology edges for a generation from Marlin's unified storage backend.
    pub fn load_topology_edges(
        &self,
        project_id: &SearchProjectId,
        generation_digest: &SearchGenerationDigest,
    ) -> Result<Vec<SearchTopologyEdgeRecord>, SearchStoragePortError> {
        self.storage_port
            .list_topology_edges(project_id, generation_digest)
    }

    /// Admit a pure typed Search Playbook plan projected from Marlin's Gerbil
    /// Scheme module system via native `RootedSchemeObject` ABI (zero pseudo
    /// S-expression text parsing).
    pub fn admit_search_plan(
        &self,
        plan: SearchPlaybookPlanInput,
    ) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
        admit_search_playbook_plan(plan)
    }

    /// Execute deterministic graph-owner ranking over admitted workspace evidence.
    #[must_use]
    pub fn rank_graph_owners(&self, request: GraphOwnerRankRequest) -> GraphOwnerRankReport {
        rank_graph_owner_report(request)
    }

    /// Execute deterministic lexical + structural source-index ranking.
    #[must_use]
    pub fn rank_source_index(&self, request: SourceIndexRankRequest) -> SourceIndexRankReport {
        rank_source_index_report(request)
    }

    /// Execute an admitted multi-axis Search Playbook purely in-process over
    /// `workspace_root`, returning a generation-bound deterministic report.
    pub fn execute_admitted_playbook(
        &self,
        workspace_root: &Path,
        request: &ProgressiveSearchPlaybookRequest,
    ) -> Result<ConvergedSearchExecutionReport, ConvergedSearchExecutionError> {
        execute_admitted_playbook(workspace_root, request)
    }

    /// Execute an admitted Search Playbook and atomically publish its canonical
    /// JSON artifact into Marlin's unified `SearchEngineStoragePort` via CAS.
    pub fn execute_and_publish_playbook(
        &self,
        project_id: SearchProjectId,
        workspace_id: SearchWorkspaceId,
        workspace_root: &Path,
        expected_hash: Option<&SearchArtifactHash>,
        request: &ProgressiveSearchPlaybookRequest,
    ) -> Result<
        (
            ConvergedSearchExecutionReport,
            SearchGenerationPointerRecord,
        ),
        ConvergedSearchExecutionError,
    > {
        let report = self.execute_admitted_playbook(workspace_root, request)?;
        let body = serde_json::to_vec(&report).map_err(|err| {
            ConvergedSearchExecutionError::Storage(SearchStoragePortError::Backend(err.to_string()))
        })?;
        let pointer = self.publish_generation_artifact(
            project_id,
            workspace_id,
            expected_hash,
            report.generation_digest.clone(),
            "application/vnd.asp.converged-search-report.v1+json",
            body,
        )?;
        Ok((report, pointer))
    }
}
