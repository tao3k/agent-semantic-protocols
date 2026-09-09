// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Deterministic from-scratch construction of one Project Topology generation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use agent_semantic_content_identity::{CanonicalItemSelector, ProjectWorkspaceBinding};
use serde_json::{Value, json};

use crate::project_topology_generation_error::{
    ProjectTopologyGenerationBuildError, error, require_digest, validate_identifier,
};
use crate::{
    ProjectTopologyClosureBuilder, ProjectTopologyClosureLimits, ProjectTopologyDirectEdge,
    ProjectTopologyExpectedRelation, ProjectTopologyInferenceProgram, ProjectTopologyLibrary,
    ProjectTopologyLibraryError, project_topology_frontier::project_topology_frontier_projection,
};

const CONSUMERS: [&str; 6] = [
    "search",
    "query",
    "code-understanding",
    "framework-calibration",
    "refactoring",
    "context-recovery",
];

const MIN_TOPOLOGY_BLOCKING_PARALLELISM: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceNode {
    id: String,
    locator: ProjectTopologySourceNodeLocator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectTopologySourceNodeLocator {
    Selector(CanonicalItemSelector),
    Owner {
        language_id: String,
        owner_path: String,
    },
}

impl ProjectTopologySourceNode {
    pub fn new(
        id: impl Into<String>,
        selector: impl Into<String>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let id = id.into();
        validate_identifier(&id, "node identity")?;
        let selector = CanonicalItemSelector::parse(selector.into()).map_err(|cause| {
            error(
                "topology-generation-selector-invalid",
                format!("cannot decode parser-owned selector: {cause}"),
            )
        })?;
        Ok(Self {
            id,
            locator: ProjectTopologySourceNodeLocator::Selector(selector),
        })
    }

    pub fn new_owner(
        id: impl Into<String>,
        language_id: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let id = id.into();
        validate_identifier(&id, "node identity")?;
        let language_id = language_id.into();
        if language_id.is_empty()
            || !language_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'-' | b'_' | b'.' | b'+')
            })
        {
            return Err(error(
                "topology-generation-owner-language-invalid",
                "owner locator language must be canonical",
            ));
        }
        let owner_path = owner_path.into();
        validate_owner_path(&owner_path)?;
        Ok(Self {
            id,
            locator: ProjectTopologySourceNodeLocator::Owner {
                language_id,
                owner_path,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceSegment {
    owner_path: String,
    content_digest: String,
    nodes: Vec<ProjectTopologySourceNode>,
    edges: Vec<ProjectTopologyDirectEdge>,
}

impl ProjectTopologySourceSegment {
    pub fn new(
        owner_path: impl Into<String>,
        content_digest: impl Into<String>,
        mut nodes: Vec<ProjectTopologySourceNode>,
        mut edges: Vec<ProjectTopologyDirectEdge>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let owner_path = owner_path.into();
        validate_owner_path(&owner_path)?;
        let content_digest = content_digest.into();
        require_digest(&content_digest, "content digest")?;
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        edges.sort_by(|left, right| left.id().cmp(right.id()));
        ensure_unique(nodes.iter().map(|node| node.id.as_str()), "node")?;
        ensure_unique(edges.iter().map(ProjectTopologyDirectEdge::id), "edge")?;
        Ok(Self {
            owner_path,
            content_digest,
            nodes,
            edges,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyGenerationIdentity {
    project_workspace: ProjectWorkspaceBinding,
    source_generation_digest: String,
    provider_catalog_digest: String,
    inference_program: Arc<ProjectTopologyInferenceProgram>,
    provider_grammar_digest: String,
    resolver_digest: String,
    topology_schema_digest: String,
}

impl ProjectTopologyGenerationIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_workspace: ProjectWorkspaceBinding,
        source_generation_digest: String,
        provider_catalog_digest: String,
        inference_program: Arc<ProjectTopologyInferenceProgram>,
        provider_grammar_digest: String,
        resolver_digest: String,
        topology_schema_digest: String,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        project_workspace
            .validate()
            .map_err(|cause| error("topology-generation-workspace-invalid", cause.to_string()))?;
        require_digest(inference_program.digest(), "inference program")?;
        for (name, digest) in [
            ("source generation", &source_generation_digest),
            ("provider catalog", &provider_catalog_digest),
            ("provider grammar", &provider_grammar_digest),
            ("resolver", &resolver_digest),
            ("topology schema", &topology_schema_digest),
        ] {
            require_digest(digest, name)?;
        }
        Ok(Self {
            project_workspace,
            source_generation_digest,
            provider_catalog_digest,
            inference_program,
            provider_grammar_digest,
            resolver_digest,
            topology_schema_digest,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ProjectTopologyGenerationCandidate {
    packet: Value,
    rebuild_receipt: Value,
    rebuild_receipt_id: String,
    project_workspace: ProjectWorkspaceBinding,
}

impl ProjectTopologyGenerationCandidate {
    pub fn packet(&self) -> &Value {
        &self.packet
    }

    pub fn rebuild_receipt(&self) -> &Value {
        &self.rebuild_receipt
    }

    pub fn rebuild_receipt_id(&self) -> &str {
        &self.rebuild_receipt_id
    }

    pub fn admit(
        self,
        independently_admitted_receipts: &BTreeMap<String, Value>,
    ) -> Result<ProjectTopologyLibrary, ProjectTopologyLibraryError> {
        ProjectTopologyLibrary::admit_with_receipts(
            self.packet,
            &self.project_workspace,
            independently_admitted_receipts,
        )
    }
}

#[derive(Clone, Debug)]
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
        )
        .await
    }

    async fn spawn_build(
        &self,
        segments: Vec<ProjectTopologySourceSegment>,
        transition: ProjectTopologyGenerationTransition,
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
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            build_from_scratch_blocking(identity, limits, segments, expected_relations, transition)
        })
        .await
        .map_err(|cause| {
            error(
                "topology-generation-task-join-failed",
                format!("topology blocking task did not return a terminal: {cause}"),
            )
        })?
    }
}

#[derive(Clone, Debug)]
struct ProjectTopologyGenerationTransition {
    parent_generation_digest: Option<String>,
    rebuilt_owner_paths: BTreeSet<String>,
    previous_node_ids: BTreeSet<String>,
    previous_edge_ids: BTreeSet<String>,
    change_set_digest: Option<String>,
}

fn build_from_scratch_blocking(
    identity: ProjectTopologyGenerationIdentity,
    limits: ProjectTopologyClosureLimits,
    mut segments: Vec<ProjectTopologySourceSegment>,
    expected_relations: Vec<ProjectTopologyExpectedRelation>,
    transition: ProjectTopologyGenerationTransition,
) -> Result<ProjectTopologyGenerationCandidate, ProjectTopologyGenerationBuildError> {
    if segments.is_empty() {
        return Err(error(
            "topology-generation-empty",
            "a Project Topology generation requires at least one source segment",
        ));
    }
    segments.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    ensure_unique(
        segments.iter().map(|segment| segment.owner_path.as_str()),
        "source segment",
    )?;
    let all_node_ids = segments
        .iter()
        .flat_map(|segment| segment.nodes.iter().map(|node| node.id.as_str()))
        .collect::<Vec<_>>();
    ensure_unique(all_node_ids.iter().copied(), "global node")?;
    let all_nodes = all_node_ids.into_iter().collect::<BTreeSet<_>>();
    for expected in &expected_relations {
        if !all_nodes.contains(expected.anchor.as_str())
            || !all_nodes.contains(expected.target.as_str())
        {
            return Err(error(
                "topology-expected-relation-endpoint-unresolved",
                format!("expected relation {} has an unknown endpoint", expected.id),
            ));
        }
    }
    let all_edges = segments
        .iter()
        .flat_map(|segment| segment.edges.iter())
        .collect::<Vec<_>>();
    ensure_unique(all_edges.iter().map(|edge| edge.id()), "global edge")?;
    for edge in &all_edges {
        if !all_nodes.contains(edge.from()) || !all_nodes.contains(edge.to()) {
            return Err(error(
                "topology-generation-dangling-edge",
                format!("edge {} has an unknown endpoint", edge.id()),
            ));
        }
    }

    let closure =
        ProjectTopologyClosureBuilder::new(limits, Arc::clone(&identity.inference_program)).build(
            &identity.source_generation_digest,
            &all_edges.into_iter().cloned().collect::<Vec<_>>(),
        )?;
    let structural_digest = digest_json(&json!({
        "segments": segments.iter().map(source_segment_digest_shape).collect::<Vec<_>>()
    }));
    let frontier_projection = project_topology_frontier_projection(
        &expected_relations,
        &[],
        &identity.source_generation_digest,
    );
    let expectation_digest = frontier_projection.expectation_digest;
    let semantic_digest = digest_parts([
        "source-owned-topology-expectations",
        &structural_digest,
        &expectation_digest,
    ]);
    let mut derived_edges = Vec::new();
    let mut proof_dependencies = Vec::new();
    for relationship in closure
        .relationships()
        .iter()
        .filter(|relationship| relationship.premise_edge_ids().len() > 1)
    {
        let relationship_identity = digest_parts(
            std::iter::once(relationship.relation())
                .chain(std::iter::once(relationship.rule_id()))
                .chain(std::iter::once(relationship.from()))
                .chain(std::iter::once(relationship.to()))
                .chain(relationship.premise_edge_ids().iter().map(String::as_str)),
        );
        let suffix = &relationship_identity["blake3-256:".len()..][..20];
        let edge_id = format!("derived-{suffix}");
        let proof_ref = format!("proof-{suffix}");
        derived_edges.push(json!({
            "id": edge_id,
            "segmentId": null,
            "from": relationship.from(),
            "to": relationship.to(),
            "relation": relationship.relation(),
            "modality": "derived",
            "bindingDigest": identity.inference_program.digest(),
            "witnesses": relationship.premise_edge_ids(),
            "proofRef": proof_ref
        }));
        proof_dependencies.push(json!({
            "derivedEdgeId": edge_id,
            "premiseEdgeIds": relationship.premise_edge_ids()
        }));
    }
    derived_edges.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    proof_dependencies.sort_by(|left, right| {
        left["derivedEdgeId"]
            .as_str()
            .cmp(&right["derivedEdgeId"].as_str())
    });
    let proof_dag_digest = digest_json(&Value::Array(proof_dependencies.clone()));
    let closure_digest = closure.receipt().receipt_digest().to_owned();
    let topology_generation_digest = digest_parts([
        identity.source_generation_digest.as_str(),
        identity.provider_catalog_digest.as_str(),
        structural_digest.as_str(),
        semantic_digest.as_str(),
        identity.inference_program.digest(),
        closure_digest.as_str(),
    ]);
    let library_digest = digest_parts([
        topology_generation_digest.as_str(),
        structural_digest.as_str(),
        semantic_digest.as_str(),
        closure_digest.as_str(),
        proof_dag_digest.as_str(),
    ]);
    let change_set_digest = transition
        .change_set_digest
        .unwrap_or_else(|| structural_digest.clone());

    let mut node_records = Vec::new();
    let mut direct_edge_records = Vec::new();
    let mut segment_records = Vec::new();
    for segment in &segments {
        let segment_id = segment_id(&segment.owner_path);
        let skeleton_digest = digest_json(&source_segment_digest_shape(segment));
        let locator_digest = digest_parts(["source-locator", segment.owner_path.as_str()]);
        let segment_nodes = segment
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let segment_edges = segment
            .edges
            .iter()
            .map(|edge| edge.id().to_owned())
            .collect::<Vec<_>>();
        let scc_digest = segment_scc_digest(&segment_nodes, closure.relationships());
        for node in &segment.nodes {
            let record = match &node.locator {
                ProjectTopologySourceNodeLocator::Selector(selector) => json!({
                    "id": node.id,
                    "segmentId": segment_id,
                    "plane": "structural",
                    "language": selector.language_id.as_str(),
                    "kind": selector.kind.as_str(),
                    "name": selector.symbol.as_str(),
                    "selector": selector.structural_selector
                }),
                ProjectTopologySourceNodeLocator::Owner {
                    language_id,
                    owner_path,
                } => json!({
                    "id": node.id,
                    "segmentId": segment_id,
                    "plane": "structural",
                    "language": language_id,
                    "kind": "Owner",
                    "name": owner_path,
                    "ownerLocator": format!("{language_id}://{owner_path}")
                }),
            };
            node_records.push(record);
        }
        for edge in &segment.edges {
            direct_edge_records.push(json!({
                "id": edge.id(),
                "segmentId": segment_id,
                "from": edge.from(),
                "to": edge.to(),
                "relation": edge.relation(),
                "modality": "parser-direct",
                "bindingDigest": skeleton_digest,
                "witnesses": [edge.id()]
            }));
        }
        segment_records.push(json!({
            "id": segment_id,
            "ownerPath": segment.owner_path,
            "contentDigest": segment.content_digest,
            "skeletonDigest": skeleton_digest,
            "locatorDigest": locator_digest,
            "sccDigest": scc_digest,
            "stratum": 0,
            "nodeIds": segment_nodes,
            "edgeIds": segment_edges
        }));
    }
    node_records.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    direct_edge_records.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let mut edge_records = direct_edge_records;
    edge_records.extend(derived_edges);
    edge_records.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let frontier_projection = project_topology_frontier_projection(
        &expected_relations,
        &edge_records,
        &identity.source_generation_digest,
    );
    let successor_node_ids = node_records
        .iter()
        .filter_map(|node| node["id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let successor_edge_ids = edge_records
        .iter()
        .filter_map(|edge| edge["id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let removed_node_ids = transition
        .previous_node_ids
        .difference(&successor_node_ids)
        .cloned()
        .collect::<Vec<_>>();
    let removed_edge_ids = transition
        .previous_edge_ids
        .difference(&successor_edge_ids)
        .cloned()
        .collect::<Vec<_>>();
    let rebuilt_segment_ids = transition
        .rebuilt_owner_paths
        .iter()
        .map(|owner_path| segment_id(owner_path))
        .collect::<Vec<_>>();
    let derived_edge_ids = proof_dependencies
        .iter()
        .map(|dependency| dependency["derivedEdgeId"].clone())
        .collect::<Vec<_>>();
    let rebuild_receipt_id = format!(
        "rebuild-{}",
        &topology_generation_digest["blake3-256:".len()..][..20]
    );
    let rebuild_receipt = json!({
        "id": rebuild_receipt_id,
        "sourceGenerationDigest": identity.source_generation_digest,
        "inferenceProgramDigest": identity.inference_program.digest(),
        "topologyGenerationDigest": topology_generation_digest,
        "recomputedLibraryDigest": library_digest,
        "authority": "project-topology-rebuild.v1",
        "state": "admitted"
    });
    let packet = json!({
        "schemaId": "agent.semantic-protocols.project-topology-library",
        "schemaVersion": "1",
        "projectWorkspace": identity.project_workspace,
        "sourceGenerationDigest": identity.source_generation_digest,
        "providerCatalogDigest": identity.provider_catalog_digest,
        "libraryDigest": library_digest,
        "generation": {
            "generationDigest": topology_generation_digest,
            "parentGenerationDigest": transition.parent_generation_digest,
            "state": "complete",
            "changeSetDigest": change_set_digest,
            "rebuiltSegmentIds": rebuilt_segment_ids,
            "removedNodeIds": removed_node_ids,
            "removedEdgeIds": removed_edge_ids,
            "fromScratchEquivalentDigest": library_digest
        },
        "fromScratchRebuildReceipt": rebuild_receipt,
        "semanticAdmissionReceipts": [],
        "identities": {
            "structuralTopologyDigest": structural_digest,
            "semanticTopologyDigest": semantic_digest,
            "inferenceProgramDigest": identity.inference_program.digest(),
            "providerGrammarDigest": identity.provider_grammar_digest,
            "resolverDigest": identity.resolver_digest,
            "topologySchemaDigest": identity.topology_schema_digest
        },
        "segments": segment_records,
        "nodes": node_records,
        "edges": edge_records,
        "expectedRelations": frontier_projection.expectation_records,
        "coverageCertificates": frontier_projection.coverage_certificates,
        "frontiers": frontier_projection.frontiers,
        "closure": {
            "state": "stable",
            "digest": closure_digest,
            "proofDagDigest": proof_dag_digest,
            "derivedEdgeIds": derived_edge_ids,
            "proofDependencies": proof_dependencies
        },
        "consumers": CONSUMERS,
        "terminal": {"state": "ready", "terminalCount": 1}
    });
    Ok(ProjectTopologyGenerationCandidate {
        packet,
        rebuild_receipt,
        rebuild_receipt_id,
        project_workspace: identity.project_workspace.clone(),
    })
}

fn source_segment_digest_shape(segment: &ProjectTopologySourceSegment) -> Value {
    json!({
        "ownerPath": segment.owner_path,
        "contentDigest": segment.content_digest,
        "nodes": segment.nodes.iter().map(|node| match &node.locator {
            ProjectTopologySourceNodeLocator::Selector(selector) => json!({
                "id": node.id,
                "selector": selector.structural_selector
            }),
            ProjectTopologySourceNodeLocator::Owner { language_id, owner_path } => json!({
                "id": node.id,
                "ownerLocator": format!("{language_id}://{owner_path}")
            }),
        }).collect::<Vec<_>>(),
        "edges": segment.edges
    })
}

fn validate_incremental_predecessor(
    identity: &ProjectTopologyGenerationIdentity,
    predecessor: &ProjectTopologyLibrary,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    let identities = predecessor
        .as_json()
        .get("identities")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                "predecessor identities are unavailable",
            )
        })?;
    let expected = [
        (
            "inferenceProgramDigest",
            identity.inference_program.digest(),
        ),
        (
            "providerGrammarDigest",
            identity.provider_grammar_digest.as_str(),
        ),
        ("resolverDigest", identity.resolver_digest.as_str()),
        (
            "topologySchemaDigest",
            identity.topology_schema_digest.as_str(),
        ),
    ];
    if predecessor.project_workspace() != &identity.project_workspace
        || predecessor.provider_catalog_digest() != identity.provider_catalog_digest
        || expected
            .iter()
            .any(|(field, value)| identities.get(*field).and_then(Value::as_str) != Some(*value))
    {
        return Err(error(
            "topology-incremental-predecessor-identity-mismatch",
            "incremental reuse requires the exact workspace, provider, grammar, resolver, schema, and inference identities",
        ));
    }
    Ok(())
}

fn packet_record_ids(
    packet: &Value,
    field: &'static str,
) -> Result<BTreeSet<String>, ProjectTopologyGenerationBuildError> {
    packet
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                format!("predecessor {field} are unavailable"),
            )
        })?
        .iter()
        .map(|record| {
            record
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| {
                    error(
                        "topology-incremental-predecessor-invalid",
                        format!("predecessor {field} contain an invalid identity"),
                    )
                })
        })
        .collect()
}

fn rehydrate_source_segments(
    predecessor: &ProjectTopologyLibrary,
) -> Result<Vec<ProjectTopologySourceSegment>, ProjectTopologyGenerationBuildError> {
    let packet = predecessor.as_json();
    let nodes = packet
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                "predecessor nodes are unavailable",
            )
        })?
        .iter()
        .filter(|node| node.get("segmentId").is_some_and(Value::is_string))
        .map(|node| {
            let id = required_packet_text(node, "id")?;
            let selector = required_packet_text(node, "selector")?;
            Ok((id.to_owned(), ProjectTopologySourceNode::new(id, selector)?))
        })
        .collect::<Result<BTreeMap<_, _>, ProjectTopologyGenerationBuildError>>()?;
    let edges = packet
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                "predecessor edges are unavailable",
            )
        })?
        .iter()
        .filter(|edge| edge.get("modality").and_then(Value::as_str) == Some("parser-direct"))
        .map(|edge| {
            let id = required_packet_text(edge, "id")?;
            Ok((
                id.to_owned(),
                ProjectTopologyDirectEdge::new(
                    id,
                    required_packet_text(edge, "relation")?,
                    required_packet_text(edge, "from")?,
                    required_packet_text(edge, "to")?,
                )?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ProjectTopologyGenerationBuildError>>()?;
    packet
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                "predecessor segments are unavailable",
            )
        })?
        .iter()
        .map(|segment| {
            let owner_path = required_packet_text(segment, "ownerPath")?;
            let content_digest = required_packet_text(segment, "contentDigest")?;
            let segment_nodes = required_packet_ids(segment, "nodeIds")?
                .into_iter()
                .map(|id| {
                    nodes.get(&id).cloned().ok_or_else(|| {
                        error(
                            "topology-incremental-predecessor-invalid",
                            format!("segment references unavailable node {id}"),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let segment_edges = required_packet_ids(segment, "edgeIds")?
                .into_iter()
                .map(|id| {
                    edges.get(&id).cloned().ok_or_else(|| {
                        error(
                            "topology-incremental-predecessor-invalid",
                            format!("segment references unavailable direct edge {id}"),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            ProjectTopologySourceSegment::new(
                owner_path,
                content_digest,
                segment_nodes,
                segment_edges,
            )
        })
        .collect()
}

fn required_packet_text<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, ProjectTopologyGenerationBuildError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        error(
            "topology-incremental-predecessor-invalid",
            format!("predecessor {field} is unavailable"),
        )
    })
}

fn required_packet_ids(
    value: &Value,
    field: &'static str,
) -> Result<Vec<String>, ProjectTopologyGenerationBuildError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "topology-incremental-predecessor-invalid",
                format!("predecessor {field} are unavailable"),
            )
        })?
        .iter()
        .map(|id| {
            id.as_str().map(str::to_owned).ok_or_else(|| {
                error(
                    "topology-incremental-predecessor-invalid",
                    format!("predecessor {field} contain a non-string identity"),
                )
            })
        })
        .collect()
}

fn validate_owner_path(owner_path: &str) -> Result<(), ProjectTopologyGenerationBuildError> {
    if owner_path.is_empty()
        || owner_path.starts_with('/')
        || owner_path.contains('\\')
        || owner_path.split('/').any(|part| part == "..")
    {
        return Err(error(
            "topology-generation-owner-path-invalid",
            "source segment owner path must be normalized and repository-relative",
        ));
    }
    Ok(())
}

fn segment_scc_digest(
    nodes: &[String],
    relationships: &[crate::ProjectTopologyRelationship],
) -> String {
    let components = nodes
        .iter()
        .map(|node| {
            let mut component = relationships
                .iter()
                .filter(|forward| forward.from() == node)
                .filter(|forward| {
                    relationships
                        .iter()
                        .any(|backward| backward.from() == forward.to() && backward.to() == node)
                })
                .map(|relationship| relationship.to().to_owned())
                .collect::<Vec<_>>();
            component.push(node.clone());
            component.sort();
            component.dedup();
            component
        })
        .collect::<Vec<_>>();
    digest_json(&json!(components))
}

fn segment_id(owner_path: &str) -> String {
    let digest = blake3::hash(owner_path.as_bytes()).to_hex().to_string();
    format!("segment-{}", &digest[..20])
}

fn digest_json(value: &Value) -> String {
    digest_parts([serde_json::to_string(value)
        .expect("JSON value serializes")
        .as_str()])
}

fn digest_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn ensure_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    kind: &str,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    let mut unique = BTreeSet::new();
    for value in values {
        if !unique.insert(value) {
            return Err(error(
                "topology-generation-identity-duplicate",
                format!("duplicate {kind} identity {value}"),
            ));
        }
    }
    Ok(())
}
