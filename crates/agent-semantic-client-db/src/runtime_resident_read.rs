// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use crate::runtime_server_opentelemetry::{
    RuntimePerformanceObservation, try_record_to_active_runtime,
};
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceExactProjectionDataPlaneClient, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeMerkleOwnerRead, WorkspaceRuntimeSelectorRead,
    WorkspaceSearchGenerationDataPlaneClient,
};

/// A process-local, immutable view of one published Runtime generation.
///
/// Opening the view belongs to admission/setup. Every read method is
/// synchronous and cannot perform socket, filesystem, database, provider, or
/// scheduler work.
pub struct RuntimeResidentReadClient {
    exact_projection: Option<WorkspaceExactProjectionDataPlaneClient>,
    resident_lease: Option<crate::runtime_server_workspace::WorkspaceGenerationLease>,
    search_projection: WorkspaceSearchGenerationDataPlaneClient,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResidentReadWorkCounters {
    pub provider_process_count: u64,
    pub scheduler_task_count: u64,
    pub filesystem_read_count: u64,
    pub database_read_count: u64,
    pub socket_operation_count: u64,
}

impl RuntimeResidentReadClient {
    /// Returns the owner-attributed parser topology inputs from this exact
    /// immutable generation without filesystem, DB, socket, or provider work.
    pub fn topology_source_segments(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologySourceSegment>, String> {
        self.search_projection.topology_source_segments()
    }

    pub async fn open(pointer_path: &Path, project_root: &Path) -> Result<Self, String> {
        Ok(Self {
            exact_projection: Some(
                WorkspaceExactProjectionDataPlaneClient::open(pointer_path).await?,
            ),
            resident_lease: None,
            search_projection: WorkspaceSearchGenerationDataPlaneClient::open(
                pointer_path,
                project_root,
            )
            .await?,
        })
    }

    /// Open the same immutable read surface directly from the validated
    /// process-resident generation. Durable mmap publication is an attachment,
    /// not a prerequisite for cold Search readiness.
    pub fn from_resident_lease(
        lease: crate::runtime_server_workspace::WorkspaceGenerationLease,
    ) -> Result<Self, String> {
        lease.generation().validate()?;
        Ok(Self {
            exact_projection: None,
            search_projection: WorkspaceSearchGenerationDataPlaneClient::from_generation(
                lease.generation_arc(),
            )?,
            resident_lease: Some(lease),
        })
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(exact), None) => {
                exact.read_runtime_selector(projection_kind, structural_selector)
            }
            (None, Some(lease)) => {
                lease.read_runtime_selector(projection_kind, structural_selector)
            }
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    pub fn owner_snapshot(
        &self,
        owner_path: &str,
    ) -> Result<Option<WorkspaceOwnerSnapshot>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(exact), None) => exact.owner_snapshot(owner_path),
            (None, Some(lease)) => Ok(lease
                .runtime_owner_snapshot(owner_path)
                .map(|(_, owner)| owner)),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    pub fn read_runtime_owner(
        &self,
        owner_path: &str,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String> {
        let generation_digest = self.generation_digest();
        let root_digest = self.owner_merkle_root_digest();
        Ok(match self.owner_snapshot(owner_path)? {
            Some(owner) => crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner {
                generation_digest,
                root_digest,
                owner,
            },
            None => crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest,
                root_digest,
            },
        })
    }

    /// Read bounded owner-search inputs without copying source or projections.
    pub fn read_runtime_owner_search(
        &self,
        owner_path: &str,
        query_terms: &[String],
        limit: usize,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead, String> {
        let generation_digest = self.generation_digest();
        let root_digest = self.owner_merkle_root_digest();
        let owner = match (&self.exact_projection, &self.resident_lease) {
            (Some(exact), None) => exact.owner_search_snapshot(owner_path, query_terms, limit)?,
            (None, Some(lease)) => {
                lease.runtime_owner_search_snapshot(owner_path, query_terms, limit)?
            }
            _ => return Err("Runtime resident read authority is inconsistent".to_owned()),
        };
        Ok(match owner {
            Some(owner) => {
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::Owner {
                    generation_digest,
                    root_digest,
                    owner,
                }
            }
            None => {
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::OwnerMissing {
                    generation_digest,
                    root_digest,
                }
            }
        })
    }

    pub fn read_merkle_owner(
        &self,
        owner_path: &str,
    ) -> Result<WorkspaceRuntimeMerkleOwnerRead, String> {
        self.search_projection.read_merkle_owner(owner_path)
    }

    pub fn read_source_index(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_source_index(query, authority, limit)
    }

    pub fn read_source_index_for_owner_scope(
        &self,
        query: &str,
        owner_path: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_source_index_for_owner_scope(query, owner_path, authority, limit)
    }

    pub fn read_source_index_for_language(
        &self,
        query: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_source_index_for_language(query, language_id, limit)
    }

    pub fn read_tantivy_for_language(
        &self,
        expression: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_tantivy_for_language(expression, language_id, limit)
    }

    pub fn read_cold_rg_candidates(
        &self,
        query: &str,
        owner_paths: &[String],
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_cold_rg_candidates(query, owner_paths, authority, limit)
    }

    #[must_use]
    pub fn cold_rg_corpus(&self) -> &agent_semantic_search::ColdRgCorpusArtifact {
        self.search_projection.cold_rg_corpus()
    }

    pub fn read_byte_evidence(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_byte_evidence(query, authority, limit)
    }

    pub fn read_byte_evidence_for_owner_scope(
        &self,
        query: &str,
        owner_path: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_byte_evidence_for_owner_scope(query, owner_path, authority, limit)
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        self.search_projection
            .parser_owned_callable_selector_pairs(owner_paths)
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
        self.search_projection
            .native_syntax_playbook_projection(owner_paths)
    }

    #[must_use]
    pub fn indexed_owner_count(&self) -> usize {
        self.search_projection.indexed_owner_count()
    }

    #[must_use]
    pub fn indexed_owner_paths(&self) -> Vec<String> {
        self.search_projection.indexed_owner_paths()
    }

    pub fn search_generation_authority(
        &self,
    ) -> &crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority {
        self.search_projection.authority()
    }

    pub fn graph_generation(
        &self,
    ) -> Result<Option<&agent_semantic_search::ResidentGraphGeneration>, String> {
        self.search_projection.graph_generation()
    }

    #[must_use]
    pub fn graph_generation_is_ready(&self) -> bool {
        self.search_projection.graph_generation_is_ready()
    }

    pub fn build_graph_attachment(
        &self,
        expected_content_generation_digest: &str,
    ) -> Result<crate::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming, String> {
        self.search_projection
            .build_graph_attachment(expected_content_generation_digest)
    }

    pub fn build_lexical_attachment(
        &self,
        expected_content_generation_digest: &str,
        resources: agent_semantic_search::ResidentIndexBuildResources,
    ) -> Result<crate::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming, String> {
        self.search_projection
            .build_lexical_attachment(expected_content_generation_digest, resources)
    }

    #[must_use]
    pub fn lexical_accelerator_is_ready(&self) -> bool {
        self.search_projection.lexical_accelerator_is_ready()
    }

    #[must_use]
    pub fn derived_build_workload(&self, previous: Option<&Self>) -> (usize, usize, usize) {
        self.search_projection
            .derived_build_workload(previous.map(|previous| &previous.search_projection))
    }

    pub fn fail_derived_attachments(&self, error: &str) {
        self.search_projection.fail_derived_attachments(error);
    }

    pub fn fail_graph_attachment(&self, error: &str) {
        self.search_projection.fail_graph_attachment(error);
    }

    pub fn fail_lexical_attachment(&self, error: &str) {
        self.search_projection.fail_lexical_attachment(error);
    }

    pub fn generation_digest(&self) -> String {
        self.search_projection.authority().generation_digest.clone()
    }

    /// Digest of the source snapshot admitted into this generation.
    ///
    /// This identity is intentionally distinct from the Merkle root over the
    /// projected owner records. Generation admission compares only this
    /// source-domain digest.
    pub fn source_root_digest(&self) -> String {
        self.search_projection
            .authority()
            .source_snapshot
            .root_digest
            .clone()
    }

    /// Merkle root of the projected owner records used by exact/search reads.
    pub fn owner_merkle_root_digest(&self) -> String {
        self.search_projection
            .authority()
            .owner_merkle_root_digest
            .clone()
    }

    pub fn try_record_read_observation(
        &self,
        surface: &str,
        stage: &str,
        operation_id: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        requested_projection: &str,
        elapsed_micros: u64,
        budget_micros: u64,
        budget_status: &str,
    ) -> bool {
        let mut observation = RuntimePerformanceObservation::new(
            surface,
            stage,
            elapsed_micros,
            budget_micros,
            budget_status,
        )
        .with_operation_id(operation_id);
        observation.language_id = language_id.map(|value| value.as_str().to_owned());
        observation.generation_digest = Some(self.generation_digest());
        observation.requested_projection = Some(requested_projection.to_owned());
        try_record_to_active_runtime(observation)
    }

    pub const fn work_counters(&self) -> RuntimeResidentReadWorkCounters {
        RuntimeResidentReadWorkCounters {
            provider_process_count: 0,
            scheduler_task_count: 0,
            filesystem_read_count: 0,
            database_read_count: 0,
            socket_operation_count: 0,
        }
    }
}
