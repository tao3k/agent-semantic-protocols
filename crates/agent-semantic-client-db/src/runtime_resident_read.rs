// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;
use std::sync::Arc;

#[path = "runtime_resident_exact_descendant.rs"]
mod exact_descendant;

use agent_semantic_runtime_observability::RuntimePerformanceObservation;

use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceExactProjectionDataPlaneClient, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeMerkleOwnerRead, WorkspaceRuntimeSelectorRead,
    WorkspaceSearchGenerationDataPlaneClient,
};
use agent_semantic_runtime_observability::try_record_to_active_runtime;

/// A process-local, immutable view of one published Runtime generation.
///
/// Opening the view belongs to admission/setup. Every read method is
/// synchronous and cannot perform socket, filesystem, database, provider, or
/// scheduler work.
pub struct RuntimeResidentReadClient {
    exact_projection: Option<WorkspaceExactProjectionDataPlaneClient>,
    resident_lease: Option<crate::runtime_server_workspace::WorkspaceGenerationLease>,
    search_projection: Arc<WorkspaceSearchGenerationDataPlaneClient>,
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
    /// Whether this handle can observe the mutable, process-resident semantic
    /// owner overlay. Durable mmap handles intentionally expose only immutable
    /// generation projections.
    #[must_use]
    pub const fn has_semantic_owner_materialization_authority(&self) -> bool {
        matches!(
            (&self.exact_projection, &self.resident_lease),
            (None, Some(_))
        )
    }

    /// Returns the owner-attributed parser topology inputs from this exact
    /// immutable generation without filesystem, DB, socket, or provider work.
    pub fn topology_source_segments(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologySourceSegment>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self.search_projection.topology_source_segments(),
            (None, Some(lease)) => Ok(lease.topology_source_segments()),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    /// Returns parser topology only for the canonical bounded owner frontier.
    /// Callers must not materialize the complete workspace and filter it
    /// afterward on a request path.
    pub fn topology_source_segments_for_owner_scope(
        &self,
        owner_paths: &std::collections::BTreeSet<String>,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologySourceSegment>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self
                .search_projection
                .topology_source_segments_for_owner_scope(owner_paths),
            (None, Some(lease)) => Ok(lease.topology_source_segments_for_owner_scope(owner_paths)),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    /// Resolve only an already selected exact-selector cut. This is the
    /// ordinary non-graph Search topology source and never scans all selectors
    /// belonging to a large owner.
    pub fn topology_source_segments_for_selector_scope(
        &self,
        selectors: &std::collections::BTreeSet<String>,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologySourceSegment>, String> {
        let mut by_owner = std::collections::BTreeMap::<
            String,
            crate::runtime_server_workspace::WorkspaceTopologySourceSegment,
        >::new();
        for selector in selectors {
            let hit = match (&self.exact_projection, &self.resident_lease) {
                (Some(_), None) => self.search_projection.exact_topology_selector(selector),
                (None, Some(lease)) => lease.exact_topology_selector(selector)?,
                _ => return Err("Runtime resident read authority is inconsistent".to_owned()),
            }
            .ok_or_else(|| format!("selected topology selector is unavailable: {selector}"))?;
            let entry = by_owner.entry(hit.owner_path.clone()).or_insert_with(|| {
                crate::runtime_server_workspace::WorkspaceTopologySourceSegment {
                    owner_path: hit.owner_path.clone(),
                    content_digest: hit.owner_content_digest.clone(),
                    authority: None,
                    selectors: Vec::new(),
                    relations: Vec::new(),
                }
            });
            if entry.content_digest != hit.owner_content_digest {
                return Err(format!(
                    "selected topology owner content drift: {}",
                    hit.owner_path
                ));
            }
            entry.selectors.push(selector.clone());
        }
        Ok(by_owner.into_values().collect())
    }

    pub fn owner_paths_for_graph_entry_node_ids<'a>(
        &self,
        node_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<std::collections::BTreeSet<String>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => Ok(self
                .search_projection
                .owner_paths_for_graph_entry_node_ids(node_ids)),
            (None, Some(lease)) => Ok(lease.owner_paths_for_graph_entry_node_ids(node_ids)),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    /// Returns only the immutable Search-core topology projection.
    ///
    /// Parser materialization may enrich the process-resident exact-query
    /// overlay, but that enrichment cannot change the topology identity of the
    /// same content generation's lexical Search plane.
    pub fn search_topology_source_segments(
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
            search_projection: Arc::new(
                WorkspaceSearchGenerationDataPlaneClient::open(pointer_path, project_root).await?,
            ),
        })
    }

    /// Open the same immutable read surface directly from the validated
    /// process-resident generation. Durable mmap publication is an attachment,
    /// not a prerequisite for cold Search readiness.
    pub fn from_resident_lease(
        lease: crate::runtime_server_workspace::WorkspaceGenerationLease,
    ) -> Result<Self, String> {
        let search_projection = lease.search_data_plane();
        Ok(Self {
            exact_projection: None,
            search_projection,
            resident_lease: Some(lease),
        })
    }

    #[cfg(test)]
    pub(crate) fn shares_search_data_plane_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.search_projection, &other.search_projection)
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        if projection_kind == ExactProjectionKind::Source
            && structural_selector
                .rsplit_once('#')
                .is_some_and(|(_, fragment)| fragment.contains("/segment/"))
        {
            return self.read_exact_descendant(structural_selector);
        }
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

    /// Returns non-searchable parser inputs from the admitted generation.
    pub fn auxiliary_owner_snapshots(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (None, Some(lease)) => Ok(lease.auxiliary_owner_snapshots()),
            (Some(_), None) => {
                Err("Runtime auxiliary inputs require the process-resident generation".to_owned())
            }
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

    pub fn read_topology_index(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologyHit>, String> {
        let hits = self
            .search_projection
            .read_topology_index(query, self.search_projection.topology_node_count().max(1));
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => Ok(hits.into_iter().take(limit).collect()),
            (None, Some(lease)) => Ok(lease
                .merge_topology_hits(hits, query, limit)
                .into_iter()
                .filter(|hit| lease.topology_hit_is_current(hit))
                .take(limit)
                .collect()),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    /// Resolve an exact resident byte match without provider, scheduler, DB,
    /// filesystem, or socket work.
    pub fn smallest_enclosing_topology_anchor(
        &self,
        owner_path: &str,
        match_start: usize,
        match_end: usize,
    ) -> Result<Option<agent_semantic_topology::TopologyAnchorHitV1>, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self.search_projection.smallest_enclosing_topology_anchor(
                owner_path,
                match_start,
                match_end,
            ),
            (None, Some(lease)) => {
                lease.smallest_enclosing_topology_anchor(owner_path, match_start, match_end)
            }
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    #[must_use]
    pub fn topology_node_count(&self) -> usize {
        self.search_projection.topology_node_count()
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

    /// Query the immutable base Tantivy generation plus owner-local immutable
    /// delta segments. Newest deltas shadow older deltas and base documents by
    /// owner path, so edited bytes cannot leak a stale base hit.
    pub fn resident_tantivy_owner_paths(
        &self,
        expression: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        owner_scope: Option<&[String]>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let base_scope = owner_scope.map(|paths| {
            paths
                .iter()
                .filter(|path| self.search_projection.contains_indexed_owner(path))
                .cloned()
                .collect::<Vec<_>>()
        });
        let base = if base_scope.as_ref().is_some_and(Vec::is_empty) {
            Vec::new()
        } else {
            let result = if let Some(paths) = base_scope.as_deref() {
                self.search_projection
                    .read_tantivy_for_language_owner_scope(
                        expression,
                        language_id,
                        paths,
                        u32::try_from(limit).unwrap_or(u32::MAX).max(1),
                    )
            } else {
                self.search_projection.read_tantivy_for_language(
                    expression,
                    language_id,
                    u32::try_from(limit).unwrap_or(u32::MAX).max(1),
                )
            }?;
            result
                .hits
                .iter()
                .map(|hit| hit.owner_path.clone())
                .collect()
        };
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => Ok(base),
            (None, Some(lease)) => {
                lease.merge_tantivy_owner_paths(base, expression, language_id, owner_scope, limit)
            }
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
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

    pub fn read_tantivy_for_language_owner_scope(
        &self,
        expression: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        owner_paths: &[String],
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_tantivy_for_language_owner_scope(expression, language_id, owner_paths, limit)
    }

    pub fn read_resident_grep_candidates(
        &self,
        query: &str,
        owner_paths: &[String],
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<std::sync::Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String>
    {
        self.search_projection
            .read_resident_grep_candidates(query, owner_paths, authority, limit)
    }

    #[must_use]
    pub fn resident_grep_corpus(&self) -> &agent_semantic_search::ResidentGrepCorpusArtifact {
        self.search_projection.resident_grep_corpus()
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

    pub fn resident_byte_candidate_owner_paths(
        &self,
        literal: &[u8],
        limit: usize,
    ) -> Result<Vec<String>, String> {
        self.search_projection
            .resident_byte_candidate_owner_paths(literal, limit)
    }

    pub fn resident_grep_candidate_owner_paths(
        &self,
        plan: &agent_semantic_search::ResidentGrepCandidatePlan,
        limit: usize,
    ) -> Result<
        (
            Vec<String>,
            agent_semantic_search::ResidentByteCoverageQueryReceipt,
        ),
        String,
    > {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self
                .search_projection
                .resident_grep_candidate_owner_paths(plan, limit),
            (None, Some(lease)) => lease.resident_grep_candidate_owner_paths(plan, None, limit),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    pub fn resident_owner_bytes(&self, owner_path: &str) -> Result<Option<Arc<[u8]>>, String> {
        Ok(self
            .owner_snapshot(owner_path)?
            .map(|owner| Arc::<[u8]>::from(owner.bytes)))
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
        if self.resident_lease.is_none() {
            return self
                .search_projection
                .parser_owned_callable_selector_pairs(owner_paths);
        }
        owner_paths
            .iter()
            .map(|owner_path| {
                let owner = self.owner_snapshot(owner_path)?.ok_or_else(|| {
                    "workspace search result references a missing owner".to_owned()
                })?;
                Ok(owner.selectors.into_iter().find_map(|selector| {
                    selector
                        .derived_projections
                        .iter()
                        .any(|projection| {
                            projection.projection_kind == ExactProjectionKind::CallableSkeleton
                        })
                        .then(|| (selector.selector, owner_path.clone()))
                }))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|pairs| pairs.into_iter().flatten().collect())
    }

    #[expect(
        clippy::type_complexity,
        reason = "the V1 projection returns its three typed evidence collections"
    )]
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
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self
                .search_projection
                .native_syntax_playbook_projection(owner_paths),
            (None, Some(lease)) => lease.native_syntax_playbook_projection(owner_paths),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    pub fn semantic_owner_materialized(&self, owner_path: &str) -> Result<bool, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (None, Some(lease)) => Ok(lease.semantic_owner_materialized(owner_path)),
            (Some(_), None) => Err(
                "semantic owner materialization state requires a resident workspace lease"
                    .to_owned(),
            ),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    /// Tests one complete owner scope against the immutable resident overlay.
    ///
    /// Keeping the batch at this boundary avoids repeating authority dispatch
    /// and makes the all-or-nothing ready-scope decision explicit.
    pub fn semantic_owners_materialized(
        &self,
        owner_paths: &std::collections::BTreeSet<String>,
    ) -> Result<bool, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (None, Some(lease)) => Ok(owner_paths
                .iter()
                .all(|owner_path| lease.semantic_owner_materialized(owner_path))),
            (Some(_), None) => Err(
                "semantic owner materialization state requires a resident workspace lease"
                    .to_owned(),
            ),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
    }

    #[must_use]
    pub fn indexed_owner_count(&self) -> usize {
        self.indexed_owner_paths().len()
    }

    #[must_use]
    pub fn contains_indexed_owner(&self, owner_path: &str) -> bool {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self.search_projection.contains_indexed_owner(owner_path),
            (None, Some(lease)) => lease.runtime_owner_snapshot(owner_path).is_some(),
            _ => false,
        }
    }

    #[must_use]
    pub fn indexed_owner_paths(&self) -> Vec<String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self.search_projection.indexed_owner_paths(),
            (None, Some(lease)) => lease.indexed_owner_paths(),
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub fn contains_provider_authority(
        &self,
        language_id: &str,
        provider_id: Option<&str>,
    ) -> bool {
        self.search_projection
            .contains_provider_authority(language_id, provider_id)
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

    /// Build the exact graph attachment for a bounded, already materialized
    /// owner frontier. This keeps parser/Relation work proportional to Search
    /// candidates instead of making graph construction a full-workspace Ready
    /// barrier.
    pub fn build_graph_generation_for_owner_scope(
        &self,
        owner_paths: &std::collections::BTreeSet<String>,
    ) -> Result<agent_semantic_search::ResidentGraphGeneration, String> {
        let segments = self.topology_source_segments_for_owner_scope(owner_paths)?;
        let mut admitted = std::collections::BTreeSet::new();
        for segment in &segments {
            admitted.insert((
                agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                segment.owner_path.clone(),
            ));
            admitted.extend(segment.selectors.iter().cloned().map(|selector| {
                (
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    selector,
                )
            }));
        }
        let mut relations = Vec::new();
        for segment in &segments {
            for selector in &segment.selectors {
                relations.push(agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
                    from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                        kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                        id: segment.owner_path.clone(),
                    },
                    kind: agent_semantic_content_identity::ProviderRelationKindV1::from("CONTAINS"),
                    to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                        kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                        id: selector.clone(),
                    },
                });
            }
            relations.extend(
                segment
                    .relations
                    .iter()
                    .map(|owned| owned.relation.clone())
                    .filter(|relation| {
                        admitted.contains(&(relation.from.kind, relation.from.id.clone()))
                            && admitted.contains(&(relation.to.kind, relation.to.id.clone()))
                    }),
            );
        }
        let authority = self.search_generation_authority();
        let request =
            std::sync::Arc::new(agent_semantic_search::SearchGenerationGraphRequest::new(
                &authority.content_search_generation,
                authority.source_snapshot.clone(),
                authority.workspace_generation.clone(),
                segments.iter().map(|segment| segment.owner_path.clone()),
                relations,
            )?);
        agent_semantic_search::build_resident_graph_generation(request)
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
            .derived_build_workload(previous.map(|previous| previous.search_projection.as_ref()))
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

    /// Identity of the exact topology source visible to this read handle.
    /// Resident semantic overlays advance this digest without pretending to
    /// publish a new immutable Search generation.
    pub fn topology_source_generation_digest(&self) -> Result<String, String> {
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => Ok(self.generation_digest()),
            (None, Some(lease)) => Ok(lease.runtime_generation_digest()),
            _ => Err("Runtime resident read authority is inconsistent".to_owned()),
        }
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
        match (&self.exact_projection, &self.resident_lease) {
            (Some(_), None) => self
                .search_projection
                .authority()
                .owner_merkle_root_digest
                .clone(),
            (None, Some(lease)) => lease.owner_identity_root_digest().to_owned(),
            _ => unreachable!("Runtime resident read authority is inconsistent"),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the hot-path observation avoids allocating an intermediate record"
    )]
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
