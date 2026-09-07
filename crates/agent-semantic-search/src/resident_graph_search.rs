// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-bound graph request projection for the resident lexical frontier.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use agent_semantic_content_identity::ArtifactJson;
use agent_semantic_content_identity::ProviderRelationEndpointKindV1;
use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::hash_normalized_json;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;
use serde_json::json;

use crate::SearchGenerationConstructionStage;
use crate::SearchGenerationGraphRequest;
use crate::SearchGenerationStageReceipt;
use crate::canonical_blake3_digest;
use crate::stable_graph_node_id;

#[path = "resident_graph_search_model.rs"]
mod model;

pub use model::ResidentGraphBuildMetrics;
pub use model::ResidentGraphEvaluatedEdge;
pub use model::ResidentGraphEvaluation;
pub use model::ResidentGraphEvaluationBudget;
pub use model::ResidentGraphEvaluationRequest;
pub use model::ResidentGraphGeneration;
use model::ResidentGraphNode;
pub use model::ResidentGraphRankedNode;
pub use model::ResidentGraphSearchBudget;
pub use model::ResidentGraphSearchRequest;
pub use model::ResidentGraphSearchStage;
pub use model::ResidentGraphSearchWork;

/// Build the immutable graph payload loaded once per Runtime generation.
///
/// Owner nodes are admitted even when they have no relations, so any lexical
/// frontier can be expressed as a entry-node set without changing the generation
/// graph. Relations remain parser/provider-owned facts.
pub(crate) fn materialize_resident_graph_generation(
    source_snapshot: &SourceSnapshotEvidence,
    workspace_generation: &WorkspaceGenerationEvidenceV1,
    owner_paths: impl IntoIterator<Item = String>,
    relations: impl IntoIterator<Item = ProviderProjectedRelation>,
) -> Result<ResidentGraphGeneration, String> {
    if source_snapshot.root_digest != workspace_generation.root_digest
        || u64::try_from(source_snapshot.leaf_count).ok() != Some(workspace_generation.leaf_count)
    {
        return Err("resident generation graph source binding mismatch".to_owned());
    }
    let owner_paths = owner_paths.into_iter().collect::<BTreeSet<_>>();
    if u64::try_from(owner_paths.len()).ok() != Some(workspace_generation.owner_count) {
        return Err("resident generation graph owner count mismatch".to_owned());
    }
    let mut relations = relations.into_iter().collect::<Vec<_>>();
    for relation in &relations {
        relation.validate()?;
        for endpoint in [&relation.from, &relation.to] {
            if endpoint.kind == ProviderRelationEndpointKindV1::Owner
                && !owner_paths.contains(&endpoint.id)
            {
                return Err(format!(
                    "resident generation graph relation references an unadmitted owner: {}",
                    endpoint.id
                ));
            }
        }
    }
    relations.sort_unstable();
    relations.dedup();
    let owner_paths = owner_paths.into_iter().collect::<Vec<_>>();
    materialize_canonical_resident_graph_generation(
        source_snapshot,
        workspace_generation,
        &owner_paths,
        &relations,
    )
}

fn materialize_canonical_resident_graph_generation(
    source_snapshot: &SourceSnapshotEvidence,
    workspace_generation: &WorkspaceGenerationEvidenceV1,
    owner_paths: &[String],
    relations: &[ProviderProjectedRelation],
) -> Result<ResidentGraphGeneration, String> {
    let owner_projection_started = std::time::Instant::now();
    let mut owner_node_ids = BTreeSet::new();
    let mut owner_paths_by_node_id = BTreeMap::new();
    let mut owner_node_ids_by_path = HashMap::with_capacity(owner_paths.len());
    let mut nodes_by_id = BTreeMap::new();
    for owner_path in owner_paths {
        let owner_node_id = stable_graph_node_id("owner", owner_path);
        owner_node_ids.insert(owner_node_id.clone());
        owner_paths_by_node_id.insert(owner_node_id.clone(), owner_path.clone());
        owner_node_ids_by_path.insert(owner_path.clone(), owner_node_id.clone());
        nodes_by_id.insert(
            owner_node_id.clone(),
            ResidentGraphNode {
                kind: "owner".to_owned(),
                owner_path: Some(owner_path.clone()),
            },
        );
    }
    let owner_projection_nanos = elapsed_nanos(owner_projection_started);
    let relation_projection_started = std::time::Instant::now();
    let mut edges = Vec::<(String, String, String)>::with_capacity(relations.len());
    for relation in relations {
        let from_id =
            insert_relation_node(&mut nodes_by_id, &owner_node_ids_by_path, &relation.from);
        let to_id = insert_relation_node(&mut nodes_by_id, &owner_node_ids_by_path, &relation.to);
        edges.push((from_id, to_id, relation.kind.to_string()));
    }
    let mut adjacency = BTreeMap::<String, Vec<ResidentGraphEvaluatedEdge>>::new();
    for (source, target, relation) in &edges {
        adjacency
            .entry(source.clone())
            .or_default()
            .push(ResidentGraphEvaluatedEdge {
                source: source.clone(),
                target: target.clone(),
                relation: relation.clone(),
            });
    }
    for outgoing in adjacency.values_mut() {
        outgoing.sort();
        outgoing.dedup();
    }
    let relation_projection_nanos = elapsed_nanos(relation_projection_started);
    let identity_hash_started = std::time::Instant::now();
    let digest = resident_graph_digest(source_snapshot, workspace_generation, &nodes_by_id, &edges);
    let identity_hash_nanos = elapsed_nanos(identity_hash_started);
    Ok(ResidentGraphGeneration {
        generation_request: None,
        digest,
        source_root_digest: source_snapshot.root_digest.clone(),
        leaf_count: workspace_generation.leaf_count,
        owner_count: workspace_generation.owner_count,
        owner_node_ids: Arc::new(owner_node_ids),
        owner_paths_by_node_id: Arc::new(owner_paths_by_node_id),
        nodes_by_id: Arc::new(nodes_by_id),
        adjacency: Arc::new(adjacency),
        rank_cache: Arc::new((0..64).map(|_| Mutex::new(Vec::new())).collect()),
        build_metrics: ResidentGraphBuildMetrics {
            owner_projection_nanos,
            relation_projection_nanos,
            identity_hash_nanos,
        },
    })
}

fn elapsed_nanos(started: std::time::Instant) -> u64 {
    started.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

/// Derive the canonical digest for one complete resident graph request.
pub fn canonical_resident_graph_generation_digest(
    request: &SearchGenerationGraphRequest,
) -> Result<String, String> {
    request.validate()?;
    Ok(materialize_resident_graph_generation(
        &request.source_snapshot,
        &request.workspace_generation,
        request.owner_paths.iter().cloned(),
        request.relations.iter().cloned(),
    )?
    .digest()
    .to_owned())
}

/// Construct the immutable base Search/Query graph in Rust from the same
/// generation-bound owner and relation facts as lexical and byte coverage.
///
/// This is a pure construction step. It performs no RPC, process launch,
/// publication, or cache mutation; the ASP Server remains the sole generation
/// publication authority.
pub fn build_resident_graph_generation(
    request: Arc<SearchGenerationGraphRequest>,
) -> Result<ResidentGraphGeneration, String> {
    request.validate()?;
    let mut generation = materialize_canonical_resident_graph_generation(
        &request.source_snapshot,
        &request.workspace_generation,
        &request.owner_paths,
        &request.relations,
    )?;
    generation.generation_request = Some(request);
    Ok(generation)
}

/// Reopen an immutable graph from the durable fused generation receipt.
pub fn open_resident_graph_generation(
    content_generation: &crate::ContentSearchGenerationReceipt,
    source_snapshot: &SourceSnapshotEvidence,
    workspace_generation: &WorkspaceGenerationEvidenceV1,
    owner_paths: impl IntoIterator<Item = String>,
    relations: impl IntoIterator<Item = ProviderProjectedRelation>,
    graph_stage: &SearchGenerationStageReceipt,
) -> Result<ResidentGraphGeneration, String> {
    content_generation.validate()?;
    graph_stage.validate_for(SearchGenerationConstructionStage::ResidentGraph)?;
    if graph_stage.identity != *content_generation.identity() {
        return Err("persisted resident graph stage content identity drift".to_owned());
    }
    if canonical_blake3_digest(&source_snapshot.root_digest)?
        != graph_stage.identity.source_root_digest
    {
        return Err("persisted resident graph stage source identity drift".to_owned());
    }
    let request = Arc::new(SearchGenerationGraphRequest::new(
        content_generation,
        source_snapshot.clone(),
        workspace_generation.clone(),
        owner_paths,
        relations,
    )?);
    let generation = build_resident_graph_generation(request)?;
    if generation.digest() != graph_stage.artifact_digest {
        return Err("persisted resident graph stage differs from resident graph bytes".to_owned());
    }
    Ok(generation)
}

/// Rank one lexical frontier entirely inside the immutable resident graph.
///
/// The traversal is deterministic and fail-closed. It performs no provider
/// call, process launch, durable read, generation mutation, or hidden retry.
pub fn rank_resident_graph_generation(
    request: ResidentGraphSearchRequest<'_>,
    budget: ResidentGraphSearchBudget,
) -> Result<ResidentGraphSearchStage, String> {
    let started = std::time::Instant::now();
    validate_generation_binding(&request)?;
    if !matches!(request.operation, "conceptual" | "relationship") {
        return Err("resident graph rank requires conceptual or relationship intent".to_owned());
    }
    if request.lexical_hits.is_empty() {
        return Err("resident graph rank requires a non-empty lexical frontier".to_owned());
    }
    if budget.max_nodes == 0 || budget.max_frontier == 0 || budget.max_results == 0 {
        return Err(
            "resident graph rank requires non-zero node, frontier, and result budgets".to_owned(),
        );
    }

    let cache_identity = resident_graph_rank_cache_key(&request, budget)?;
    if let Some((cache_key, cache_slot)) = &cache_identity {
        let cache = request.generation_graph.rank_cache[*cache_slot]
            .lock()
            .map_err(|_| "resident graph rank cache is poisoned".to_owned())?;
        if let Some(cached) = cache
            .iter()
            .find(|(stored_key, _)| stored_key == cache_key)
            .map(|(_, stage)| Arc::clone(stage))
        {
            let mut stage = (*cached).clone();
            stage.elapsed_micros = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            stage.work.cache_hit_count = 1;
            stage.work.cache_miss_count = 0;
            stage.work.cache_entry_count = cache.len();
            return Ok(stage);
        }
    }
    let mut frontier = VecDeque::<(String, usize, usize)>::new();
    let mut discovered = BTreeSet::<String>::new();
    for (candidate_rank, hit) in request.lexical_hits.iter().enumerate() {
        let node_id = stable_graph_node_id("owner", &hit.owner_path);
        if !request.generation_graph.owner_node_ids.contains(&node_id) {
            return Err(format!(
                "resident lexical owner is absent from generation graph: {}",
                hit.owner_path
            ));
        }
        if discovered.insert(node_id.clone()) {
            if frontier.len() >= budget.max_frontier {
                return Err("graph-frontier-budget-exhausted".to_owned());
            }
            frontier.push_back((node_id, 0, candidate_rank));
        }
    }

    let mut work = ResidentGraphSearchWork {
        frontier_peak: frontier.len(),
        ..ResidentGraphSearchWork::default()
    };
    let mut ranked = BTreeMap::<String, (usize, usize)>::new();
    while let Some((node_id, depth, candidate_rank)) = frontier.pop_front() {
        work.visited_nodes = work.visited_nodes.saturating_add(1);
        if work.visited_nodes > budget.max_nodes {
            return Err("graph-node-budget-exhausted".to_owned());
        }
        if let Some(owner_path) = request
            .generation_graph
            .owner_paths_by_node_id
            .get(&node_id)
        {
            ranked
                .entry(owner_path.clone())
                .and_modify(|current| *current = (*current).min((depth, candidate_rank)))
                .or_insert((depth, candidate_rank));
        }
        if let Some(outgoing) = request.generation_graph.adjacency.get(&node_id) {
            for edge in outgoing {
                work.visited_edges = work.visited_edges.saturating_add(1);
                if work.visited_edges > budget.max_edges {
                    return Err("graph-edge-budget-exhausted".to_owned());
                }
                if discovered.insert(edge.target.clone()) {
                    if frontier.len() >= budget.max_frontier {
                        return Err("graph-frontier-budget-exhausted".to_owned());
                    }
                    frontier.push_back((
                        edge.target.clone(),
                        depth.saturating_add(1),
                        candidate_rank,
                    ));
                    work.frontier_peak = work.frontier_peak.max(frontier.len());
                }
            }
        }
    }

    let mut ranked_owner_paths = ranked
        .into_iter()
        .map(|(path, (distance, candidate_rank))| (distance, candidate_rank, path))
        .collect::<Vec<_>>();
    ranked_owner_paths.sort();
    ranked_owner_paths.truncate(budget.max_results);
    let ranked_owner_paths = ranked_owner_paths
        .into_iter()
        .map(|(_, _, path)| path)
        .collect::<Vec<_>>();
    let result_artifact = ArtifactJson::from_serializable(&json!({
        "schemaId": "agent.semantic-protocols.resident-graph-rank-result",
        "schemaVersion": "1",
        "algorithm": "bounded-resident-frontier-v1",
        "generationDigest": request.generation_digest,
        "graphGenerationDigest": request.generation_graph.digest(),
        "rankedOwnerPaths": &ranked_owner_paths,
        "budget": {
            "maxNodes": budget.max_nodes,
            "maxEdges": budget.max_edges,
            "maxFrontier": budget.max_frontier,
            "maxResults": budget.max_results,
        },
        "work": {
            "visitedNodes": work.visited_nodes,
            "visitedEdges": work.visited_edges,
            "frontierPeak": work.frontier_peak,
            "providerRpcCount": work.provider_rpc_count,
        },
    }))
    .map_err(|error| format!("canonicalize resident graph rank: {error}"))?;
    let result_digest = format!(
        "blake3-256:{}",
        hash_normalized_json(&result_artifact).value
    );
    work.cache_miss_count = usize::from(cache_identity.is_some());
    work.cache_capacity = request.generation_graph.rank_cache.len() * 16;
    work.cache_shard_count = request.generation_graph.rank_cache.len();
    let mut stage = ResidentGraphSearchStage {
        generation_digest: request.generation_digest.to_owned(),
        result_digest,
        ranked_owner_paths,
        elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        work,
    };
    if let Some((cache_key, cache_slot)) = cache_identity {
        stage.work.cache_value_bytes = resident_graph_rank_cache_value_bytes(&stage);
        let mut cache = request.generation_graph.rank_cache[cache_slot]
            .lock()
            .map_err(|_| "resident graph rank cache is poisoned".to_owned())?;
        if cache.len() == 16 {
            cache.remove(0);
        }
        cache.push((cache_key, Arc::new(stage.clone())));
        stage.work.cache_entry_count = cache.len();
    }
    Ok(stage)
}

fn resident_graph_rank_cache_key(
    request: &ResidentGraphSearchRequest<'_>,
    budget: ResidentGraphSearchBudget,
) -> Result<Option<(String, usize)>, String> {
    let Some(generation_request) = request.generation_graph.generation_request.as_deref() else {
        return Ok(None);
    };
    let mut hasher = blake3::Hasher::new();
    for component in [
        generation_request.identity.project_id.as_str(),
        generation_request.identity.workspace_id.as_str(),
        generation_request.identity.source_root_digest.as_str(),
        generation_request.identity.provider_digest.as_str(),
        generation_request.identity.schema_digest.as_str(),
        generation_request
            .identity
            .generation_candidate_digest
            .as_str(),
        request.operation,
        request.language_id,
        request.provider_id,
        request.generation_digest,
    ] {
        hasher.update(component.as_bytes());
        hasher.update(&[0]);
    }
    for term in request.query.split_whitespace() {
        for byte in term.bytes() {
            hasher.update(&[byte.to_ascii_lowercase()]);
        }
        hasher.update(&[0]);
    }
    for bound in [
        budget.max_nodes,
        budget.max_edges,
        budget.max_frontier,
        budget.max_results,
    ] {
        hasher.update(&(bound as u64).to_le_bytes());
    }
    for hit in request.lexical_hits {
        for component in [
            hit.owner_path.as_str(),
            hit.owner_content_digest.as_str(),
            hit.language_id.as_deref().unwrap_or(""),
            hit.projection_tier.as_str(),
            hit.selector.as_deref().unwrap_or(""),
        ] {
            hasher.update(component.as_bytes());
            hasher.update(&[0]);
        }
        hasher.update(&hit.line_count.to_le_bytes());
        hasher.update(&hit.score.unwrap_or_default().to_le_bytes());
        for query_key in &hit.query_keys {
            hasher.update(query_key.as_bytes());
            hasher.update(&[0]);
        }
        hasher.update(&[0xff]);
    }
    let digest = hasher.finalize();
    let slot = usize::from(digest.as_bytes()[0]) % request.generation_graph.rank_cache.len();
    Ok(Some((format!("blake3-256:{digest}"), slot)))
}

fn resident_graph_rank_cache_value_bytes(stage: &ResidentGraphSearchStage) -> usize {
    stage.generation_digest.len()
        + stage.result_digest.len()
        + stage
            .ranked_owner_paths
            .iter()
            .map(String::len)
            .sum::<usize>()
        + std::mem::size_of::<ResidentGraphSearchStage>()
}

/// Evaluate explicit resident graph entry nodes without opening a provider session.
pub fn evaluate_resident_graph_generation(
    request: ResidentGraphEvaluationRequest<'_>,
    budget: ResidentGraphEvaluationBudget,
) -> Result<ResidentGraphEvaluation, String> {
    if request.operation_id.is_empty()
        || !request.generation_digest.starts_with("blake3-256:")
        || !request
            .generation_graph
            .matches_generation_binding(request.source_snapshot, request.workspace_generation)
    {
        return Err("resident graph evaluation generation binding mismatch".to_owned());
    }
    if request.entry_node_ids.is_empty() {
        return Err("resident graph evaluation requires at least one entry node".to_owned());
    }
    if budget.max_nodes == 0 || budget.max_results == 0 {
        return Err(
            "resident graph evaluation requires non-zero node and result budgets".to_owned(),
        );
    }

    let mut entry_nodes = request.entry_node_ids.to_vec();
    entry_nodes.sort();
    entry_nodes.dedup();
    if entry_nodes.iter().any(|entry_node| {
        !request
            .generation_graph
            .nodes_by_id
            .contains_key(entry_node)
    }) {
        return Err(
            "resident graph evaluation entry node is absent from the generation".to_owned(),
        );
    }

    let mut frontier = VecDeque::<(String, usize, usize)>::new();
    let mut discovered = BTreeSet::<String>::new();
    for (entry_rank, entry_node) in entry_nodes.into_iter().enumerate() {
        discovered.insert(entry_node.clone());
        frontier.push_back((entry_node, 0, entry_rank));
    }
    let mut work = ResidentGraphSearchWork {
        frontier_peak: frontier.len(),
        ..ResidentGraphSearchWork::default()
    };
    let mut ranked = Vec::<(usize, usize, String)>::new();
    let mut traversed_edges = BTreeSet::<ResidentGraphEvaluatedEdge>::new();
    while let Some((node_id, distance, candidate_rank)) = frontier.pop_front() {
        work.visited_nodes = work.visited_nodes.saturating_add(1);
        if work.visited_nodes > budget.max_nodes {
            return Err("graph-node-budget-exhausted".to_owned());
        }
        ranked.push((distance, candidate_rank, node_id.clone()));
        if distance >= budget.max_depth {
            continue;
        }
        if let Some(outgoing) = request.generation_graph.adjacency.get(&node_id) {
            for edge in outgoing {
                work.visited_edges = work.visited_edges.saturating_add(1);
                if work.visited_edges > budget.max_edges {
                    return Err("graph-edge-budget-exhausted".to_owned());
                }
                traversed_edges.insert(edge.clone());
                if discovered.insert(edge.target.clone()) {
                    frontier.push_back((edge.target.clone(), distance + 1, candidate_rank));
                    work.frontier_peak = work.frontier_peak.max(frontier.len());
                }
            }
        }
    }
    ranked.sort();
    ranked.truncate(budget.max_results);
    let ranked_nodes = ranked
        .into_iter()
        .map(|(distance, candidate_rank, id)| {
            let node = &request.generation_graph.nodes_by_id[&id];
            ResidentGraphRankedNode {
                id,
                kind: node.kind.clone(),
                owner_path: node.owner_path.clone(),
                score: budget
                    .max_depth
                    .saturating_sub(distance)
                    .saturating_mul(1024)
                    .saturating_add(1024usize.saturating_sub(candidate_rank)),
                distance,
            }
        })
        .collect();
    Ok(ResidentGraphEvaluation {
        ranked_nodes,
        edges: traversed_edges.into_iter().collect(),
        work,
    })
}

fn validate_generation_binding(request: &ResidentGraphSearchRequest<'_>) -> Result<(), String> {
    if request.operation_id.is_empty()
        || request.language_id.is_empty()
        || request.provider_id.is_empty()
        || !request.generation_digest.starts_with("blake3-256:")
        || request.source_snapshot.root_digest != request.workspace_generation.root_digest
        || u64::try_from(request.source_snapshot.leaf_count).ok()
            != Some(request.workspace_generation.leaf_count)
        || !request
            .generation_graph
            .matches_generation_binding(request.source_snapshot, request.workspace_generation)
    {
        return Err("resident graph search generation binding mismatch".to_owned());
    }
    if request
        .lexical_hits
        .iter()
        .any(|hit| hit.owner_path.is_empty() || hit.owner_content_digest.is_empty())
    {
        return Err("resident graph search contains an invalid lexical hit".to_owned());
    }
    Ok(())
}

fn insert_relation_node(
    nodes_by_id: &mut BTreeMap<String, ResidentGraphNode>,
    owner_node_ids_by_path: &HashMap<String, String>,
    endpoint: &ProviderProjectedRelationEndpoint,
) -> String {
    if endpoint.kind == ProviderRelationEndpointKindV1::Owner {
        return owner_node_ids_by_path[&endpoint.id].clone();
    }
    let node_id = stable_graph_node_id(endpoint.kind.as_str(), &endpoint.id);
    nodes_by_id
        .entry(node_id.clone())
        .or_insert_with(|| ResidentGraphNode {
            kind: endpoint.kind.to_string(),
            owner_path: None,
        });
    node_id
}

fn resident_graph_digest(
    source_snapshot: &SourceSnapshotEvidence,
    workspace_generation: &WorkspaceGenerationEvidenceV1,
    nodes: &BTreeMap<String, ResidentGraphNode>,
    edges: &[(String, String, String)],
) -> String {
    let mut hasher = blake3::Hasher::new();
    hash_graph_component(
        &mut hasher,
        "agent.semantic-protocols.resident-graph-generation.v1",
    );
    for value in [
        source_snapshot.root_digest.as_str(),
        source_snapshot.provider_digest.as_str(),
    ] {
        hash_graph_component(&mut hasher, value);
    }
    hasher.update(&workspace_generation.root_depth.to_le_bytes());
    hasher.update(&workspace_generation.leaf_count.to_le_bytes());
    hasher.update(&workspace_generation.owner_count.to_le_bytes());
    for (node_id, node) in nodes {
        hash_graph_component(&mut hasher, node_id);
        hash_graph_component(&mut hasher, &node.kind);
        match node.owner_path.as_deref() {
            Some(owner_path) => {
                hash_graph_component(&mut hasher, "owner-path");
                hash_graph_component(&mut hasher, owner_path);
            }
            None => hash_graph_component(&mut hasher, "non-owner-node"),
        }
    }
    for (source, target, relation) in edges {
        hash_graph_component(&mut hasher, source);
        hash_graph_component(&mut hasher, target);
        hash_graph_component(&mut hasher, relation);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn hash_graph_component(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}
