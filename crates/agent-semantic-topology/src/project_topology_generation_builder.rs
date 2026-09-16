// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Deterministic from-scratch construction of one Project Topology generation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::json;

use crate::project_topology_generation_error::{ProjectTopologyGenerationBuildError, error};
use crate::project_topology_generation_execution::{
    ProjectTopologyGenerationTransition, build_candidate_blocking, digest_json, packet_record_ids,
    rehydrate_source_segments, source_segment_digest_shape, validate_incremental_predecessor,
    validate_owner_path,
};
use crate::project_topology_generation_model::{
    ProjectTopologyGenerationCandidate, ProjectTopologyGenerationIdentity,
    ProjectTopologySourceSegment, ensure_unique,
};
use crate::{
    ProjectTopologyClosureLimits, ProjectTopologyExpectedRelation, ProjectTopologyLibrary,
};

const MIN_TOPOLOGY_BLOCKING_PARALLELISM: usize = 4;

pub struct ProjectTopologyGenerationBuilder {
    identity: ProjectTopologyGenerationIdentity,
    limits: ProjectTopologyClosureLimits,
    expected_relations: Vec<ProjectTopologyExpectedRelation>,
    blocking_permits: Arc<tokio::sync::Semaphore>,
}

impl ProjectTopologyGenerationBuilder {
    pub fn new(
        identity: ProjectTopologyGenerationIdentity,
        limits: ProjectTopologyClosureLimits,
    ) -> Self {
        let blocking_parallelism = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(MIN_TOPOLOGY_BLOCKING_PARALLELISM)
            .max(MIN_TOPOLOGY_BLOCKING_PARALLELISM);
        Self {
            identity,
            limits,
            expected_relations: Vec::new(),
            blocking_permits: Arc::new(tokio::sync::Semaphore::new(blocking_parallelism)),
        }
    }

    /// Attach source-owned obligations evaluated in the same generation as
    /// direct and derived relations.
    pub fn with_expected_relations(
        mut self,
        mut expected_relations: Vec<ProjectTopologyExpectedRelation>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        expected_relations.sort_by(|left, right| left.id.cmp(&right.id));
        ensure_unique(
            expected_relations
                .iter()
                .map(|expected| expected.id.as_str()),
            "expected relation",
        )?;
        self.expected_relations = expected_relations;
        Ok(self)
    }

    /// Returns the bounded CPU-build concurrency configured for this builder.
    /// This is observability only and never participates in generation identity.
    #[must_use]
    pub fn blocking_parallelism(&self) -> usize {
        self.blocking_permits.available_permits()
    }

    /// Build one immutable candidate without blocking a Tokio worker thread.
    ///
    /// Runtime owns cancellation, generation single-flight, receipt admission,
    /// and atomic publication. This builder owns neither an active-generation
    /// cache nor a publication side effect.
    pub async fn build_from_scratch(
        &self,
        segments: Vec<ProjectTopologySourceSegment>,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        self.build_initial(segments, false).await
    }

    /// Build on a caller-owned blocking lane.
    ///
    /// Runtime Server uses this entry so its daemon-wide CPU, memory, task,
    /// and cancellation authorities remain attached to the actual blocking
    /// closure. Standalone callers should normally use the async API above.
    pub fn build_from_scratch_on_blocking_lane(
        &self,
        segments: Vec<ProjectTopologySourceSegment>,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        let rebuilt_owner_paths = segments
            .iter()
            .map(|segment| segment.owner_path.clone())
            .collect::<BTreeSet<_>>();
        build_candidate_blocking(
            self.identity.clone(),
            self.limits,
            segments,
            self.expected_relations.clone(),
            ProjectTopologyGenerationTransition {
                parent_generation_digest: None,
                rebuilt_owner_paths,
                previous_node_ids: BTreeSet::new(),
                previous_edge_ids: BTreeSet::new(),
                change_set_digest: None,
            },
            false,
        )
    }

    /// Builds an identity-bound empty request cut; full generations remain non-empty.
    pub async fn build_empty_request_scope(
        &self,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        self.build_initial(Vec::new(), true).await
    }
    async fn build_initial(
        &self,
        segments: Vec<ProjectTopologySourceSegment>,
        allow_empty_request_scope: bool,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        let rebuilt_owner_paths = segments
            .iter()
            .map(|segment| segment.owner_path.clone())
            .collect::<BTreeSet<_>>();
        self.spawn_build(
            segments,
            ProjectTopologyGenerationTransition {
                parent_generation_digest: None,
                rebuilt_owner_paths,
                previous_node_ids: BTreeSet::new(),
                previous_edge_ids: BTreeSet::new(),
                change_set_digest: None,
            },
            allow_empty_request_scope,
        )
        .await
    }

    /// Build a successor from immutable predecessor segments and one explicit
    /// owner change set. Removed premise edges are retracted before closure is
    /// recomputed; no predecessor-derived relationship is copied forward.
    pub async fn build_incremental(
        &self,
        predecessor: Arc<ProjectTopologyLibrary>,
        changed_segments: Vec<ProjectTopologySourceSegment>,
        removed_owner_paths: Vec<String>,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        validate_incremental_predecessor(&self.identity, &predecessor)?;
        let previous_node_ids = packet_record_ids(predecessor.as_json(), "nodes")?;
        let previous_edge_ids = packet_record_ids(predecessor.as_json(), "edges")?;
        let mut segments = rehydrate_source_segments(&predecessor)?
            .into_iter()
            .map(|segment| (segment.owner_path.clone(), segment))
            .collect::<BTreeMap<_, _>>();
        let removed_owner_paths = removed_owner_paths.into_iter().collect::<BTreeSet<_>>();
        for owner_path in &removed_owner_paths {
            validate_owner_path(owner_path)?;
            if segments.remove(owner_path).is_none() {
                return Err(error(
                    "topology-incremental-owner-missing",
                    format!("removed owner is absent from predecessor: {owner_path}"),
                ));
            }
        }
        let mut rebuilt_owner_paths = BTreeSet::new();
        let mut changed_shapes = Vec::new();
        for segment in changed_segments {
            if removed_owner_paths.contains(&segment.owner_path) {
                return Err(error(
                    "topology-incremental-change-conflict",
                    format!(
                        "owner cannot be changed and removed in one generation: {}",
                        segment.owner_path
                    ),
                ));
            }
            rebuilt_owner_paths.insert(segment.owner_path.clone());
            changed_shapes.push(source_segment_digest_shape(&segment));
            segments.insert(segment.owner_path.clone(), segment);
        }
        let change_set_digest = digest_json(&json!({
            "changedSegments": changed_shapes,
            "removedOwnerPaths": removed_owner_paths,
        }));
        self.spawn_build(
            segments.into_values().collect(),
            ProjectTopologyGenerationTransition {
                parent_generation_digest: Some(predecessor.generation_digest().to_owned()),
                rebuilt_owner_paths,
                previous_node_ids,
                previous_edge_ids,
                change_set_digest: Some(change_set_digest),
            },
            false,
        )
        .await
    }

    async fn spawn_build(
        &self,
        segments: Vec<ProjectTopologySourceSegment>,
        transition: ProjectTopologyGenerationTransition,
        allow_empty_request_scope: bool,
    ) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
        let permit = self
            .blocking_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                error(
                    "topology-generation-runtime-closed",
                    "topology blocking-work admission is closed",
                )
            })?;
        let identity = self.identity.clone();
        let limits = self.limits;
        let expected_relations = self.expected_relations.clone();
        agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
            "project-topology-generation-build",
            move || {
                let _permit = permit;
                build_candidate_blocking(
                    identity,
                    limits,
                    segments,
                    expected_relations,
                    transition,
                    allow_empty_request_scope,
                )
            },
        )
        .join()
        .await
        .map_err(|cause| {
            error(
                "topology-generation-task-join-failed",
                format!("topology blocking task did not return a terminal: {cause}"),
            )
        })?
    }
}
