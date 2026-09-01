//! Generation-bound graph request projection for the resident lexical frontier.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use agent_semantic_content_identity::{
    ArtifactJson, SourceSnapshotEvidence, hash_normalized_json,
    provider_projection_relation::{ProviderProjectedRelation, ProviderProjectedRelationEndpoint},
    workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
};
use agent_semantic_search_projection::ResidentSearchHit;
use serde_json::{Value, json};

use crate::stable_graph_node_id;

/// Inputs admitted by one immutable Runtime search generation.
pub struct ResidentGraphSearchRequest<'a> {
    pub operation_id: &'a str,
    pub operation: &'a str,
    pub query: &'a str,
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub generation_digest: &'a str,
    pub source_snapshot: &'a SourceSnapshotEvidence,
    pub workspace_generation: &'a WorkspaceGenerationEvidenceV1,
    pub lexical_hits: &'a [ResidentSearchHit],
    /// Immutable whole-generation graph admitted with the resident mmap.
    /// Query-specific lexical hits are represented only by `seedIds`.
    pub generation_graph: &'a ResidentGraphGeneration,
}

/// Compact graph stage evidence retained by the v1 Runtime search receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphSearchStage {
    pub generation_digest: String,
    pub result_digest: String,
    pub ranked_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
    pub work: ResidentGraphSearchWork,
}

/// Explicit upper bounds for one warm resident graph traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentGraphSearchBudget {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_frontier: usize,
    pub max_results: usize,
}

/// Intent-only graph evaluation admitted against one immutable generation.
pub struct ResidentGraphEvaluationRequest<'a> {
    pub operation_id: &'a str,
    pub generation_digest: &'a str,
    pub source_snapshot: &'a SourceSnapshotEvidence,
    pub workspace_generation: &'a WorkspaceGenerationEvidenceV1,
    pub seed_ids: &'a [String],
    pub generation_graph: &'a ResidentGraphGeneration,
}

/// Explicit upper bounds for a public resident graph evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentGraphEvaluationBudget {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_results: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphRankedNode {
    pub id: String,
    pub kind: String,
    pub owner_path: Option<String>,
    pub score: usize,
    pub distance: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResidentGraphEvaluatedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphEvaluation {
    pub ranked_nodes: Vec<ResidentGraphRankedNode>,
    pub edges: Vec<ResidentGraphEvaluatedEdge>,
    pub work: ResidentGraphSearchWork,
}

/// Machine-readable work counters for the Ready-path graph stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentGraphSearchWork {
    pub visited_nodes: usize,
    pub visited_edges: usize,
    pub frontier_peak: usize,
    pub provider_rpc_count: usize,
}

/// Canonical whole-generation graph shared by all warm queries.
#[derive(Clone, Debug)]
pub struct ResidentGraphGeneration {
    open_payload: Arc<Value>,
    digest: String,
    source_root_digest: String,
    leaf_count: u64,
    owner_count: u64,
    owner_node_ids: Arc<BTreeSet<String>>,
    owner_paths_by_node_id: Arc<BTreeMap<String, String>>,
    nodes_by_id: Arc<BTreeMap<String, ResidentGraphNode>>,
    adjacency: Arc<BTreeMap<String, Vec<ResidentGraphEvaluatedEdge>>>,
}

#[derive(Clone, Debug)]
struct ResidentGraphNode {
    kind: String,
    owner_path: Option<String>,
}

impl ResidentGraphGeneration {
    #[must_use]
    pub fn graph(&self) -> &Value {
        &self.open_payload["graph"]
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn matches_generation_binding(
        &self,
        source_snapshot: &SourceSnapshotEvidence,
        workspace_generation: &WorkspaceGenerationEvidenceV1,
    ) -> bool {
        self.source_root_digest == source_snapshot.root_digest
            && self.source_root_digest == workspace_generation.root_digest
            && self.leaf_count == workspace_generation.leaf_count
            && self.owner_count == workspace_generation.owner_count
    }
}

/// Build the immutable graph payload loaded once per Runtime generation.
///
/// Owner nodes are admitted even when they have no relations, so any lexical
/// frontier can be expressed as a seed set without changing the generation
/// graph. Relations remain parser/provider-owned facts.
pub fn build_resident_graph_generation(
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
    let mut nodes = BTreeMap::<String, Value>::new();
    let mut edges = BTreeSet::<(String, String, String)>::new();
    let owner_paths = owner_paths.into_iter().collect::<BTreeSet<_>>();
    if u64::try_from(owner_paths.len()).ok() != Some(workspace_generation.owner_count) {
        return Err("resident generation graph owner count mismatch".to_owned());
    }
    let mut owner_node_ids = BTreeSet::new();
    let mut owner_paths_by_node_id = BTreeMap::new();
    for owner_path in &owner_paths {
        let owner_node_id = stable_graph_node_id("owner", owner_path);
        owner_node_ids.insert(owner_node_id.clone());
        owner_paths_by_node_id.insert(owner_node_id, owner_path.clone());
        insert_owner_node(&mut nodes, owner_path);
    }
    for relation in relations {
        relation.validate()?;
        for endpoint in [&relation.from, &relation.to] {
            if endpoint.kind == "owner" && !owner_paths.contains(&endpoint.id) {
                return Err(format!(
                    "resident generation graph relation references an unadmitted owner: {}",
                    endpoint.id
                ));
            }
        }
        let from_id = insert_relation_node(&mut nodes, &relation.from);
        let to_id = insert_relation_node(&mut nodes, &relation.to);
        edges.insert((from_id, to_id, relation.kind));
    }
    let nodes_by_id = nodes
        .iter()
        .map(|(node_id, node)| {
            let kind = node["kind"].as_str().unwrap_or("unknown").to_owned();
            let owner_path = node
                .get("ownerPath")
                .and_then(Value::as_str)
                .map(str::to_owned);
            (node_id.clone(), ResidentGraphNode { kind, owner_path })
        })
        .collect::<BTreeMap<_, _>>();
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
    let graph = json!({
        "nodes": nodes.into_values().collect::<Vec<_>>(),
        "edges": edges.into_iter().map(|(source, target, relation)| json!({
            "source": source,
            "target": target,
            "relation": relation,
        })).collect::<Vec<_>>(),
    });
    let open_payload = json!({
        "graph": graph,
        "sourceSnapshot": source_snapshot,
        "workspaceGeneration": workspace_generation,
    });
    let artifact = ArtifactJson::from_serializable(&open_payload)
        .map_err(|error| format!("canonicalize resident generation graph: {error}"))?;
    let digest = format!("blake3-256:{}", hash_normalized_json(&artifact).value);
    Ok(ResidentGraphGeneration {
        open_payload: Arc::new(open_payload),
        digest,
        source_root_digest: source_snapshot.root_digest.clone(),
        leaf_count: workspace_generation.leaf_count,
        owner_count: workspace_generation.owner_count,
        owner_node_ids: Arc::new(owner_node_ids),
        owner_paths_by_node_id: Arc::new(owner_paths_by_node_id),
        nodes_by_id: Arc::new(nodes_by_id),
        adjacency: Arc::new(adjacency),
    })
}

/// Rank one lexical frontier entirely inside the immutable resident graph.
///
/// The traversal is deterministic and fail-closed. It performs no provider
/// call, process launch, durable read, generation mutation, or hidden retry.
pub fn rank_resident_graph_generation(
    request: ResidentGraphSearchRequest<'_>,
    budget: ResidentGraphSearchBudget,
) -> Result<ResidentGraphSearchStage, String> {
    validate_generation_binding(&request)?;
    if request.operation != "pipe" {
        return Err("resident graph rank requires the pipe operation".to_owned());
    }
    if request.lexical_hits.is_empty() {
        return Err("resident graph rank requires a non-empty lexical frontier".to_owned());
    }
    if budget.max_nodes == 0 || budget.max_frontier == 0 || budget.max_results == 0 {
        return Err(
            "resident graph rank requires non-zero node, frontier, and result budgets".to_owned(),
        );
    }

    let started = std::time::Instant::now();
    let mut frontier = VecDeque::<(String, usize, usize)>::new();
    let mut discovered = BTreeSet::<String>::new();
    for (seed_rank, hit) in request.lexical_hits.iter().enumerate() {
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
            frontier.push_back((node_id, 0, seed_rank));
        }
    }

    let mut work = ResidentGraphSearchWork {
        frontier_peak: frontier.len(),
        ..ResidentGraphSearchWork::default()
    };
    let mut ranked = BTreeMap::<String, (usize, usize)>::new();
    while let Some((node_id, depth, seed_rank)) = frontier.pop_front() {
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
                .and_modify(|current| *current = (*current).min((depth, seed_rank)))
                .or_insert((depth, seed_rank));
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
                    frontier.push_back((edge.target.clone(), depth.saturating_add(1), seed_rank));
                    work.frontier_peak = work.frontier_peak.max(frontier.len());
                }
            }
        }
    }

    let mut ranked_owner_paths = ranked
        .into_iter()
        .map(|(path, (distance, seed_rank))| (distance, seed_rank, path))
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
    Ok(ResidentGraphSearchStage {
        generation_digest: request.generation_digest.to_owned(),
        result_digest,
        ranked_owner_paths,
        elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        work,
    })
}

/// Evaluate explicit resident graph seeds without opening a provider session.
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
    if request.seed_ids.is_empty() {
        return Err("resident graph evaluation requires at least one seed".to_owned());
    }
    if budget.max_nodes == 0 || budget.max_results == 0 {
        return Err(
            "resident graph evaluation requires non-zero node and result budgets".to_owned(),
        );
    }

    let mut seeds = request.seed_ids.to_vec();
    seeds.sort();
    seeds.dedup();
    if seeds
        .iter()
        .any(|seed| !request.generation_graph.nodes_by_id.contains_key(seed))
    {
        return Err("resident graph evaluation seed is absent from the generation".to_owned());
    }

    let mut frontier = VecDeque::<(String, usize, usize)>::new();
    let mut discovered = BTreeSet::<String>::new();
    for (seed_rank, seed) in seeds.into_iter().enumerate() {
        discovered.insert(seed.clone());
        frontier.push_back((seed, 0, seed_rank));
    }
    let mut work = ResidentGraphSearchWork {
        frontier_peak: frontier.len(),
        ..ResidentGraphSearchWork::default()
    };
    let mut ranked = Vec::<(usize, usize, String)>::new();
    let mut traversed_edges = BTreeSet::<ResidentGraphEvaluatedEdge>::new();
    while let Some((node_id, distance, seed_rank)) = frontier.pop_front() {
        work.visited_nodes = work.visited_nodes.saturating_add(1);
        if work.visited_nodes > budget.max_nodes {
            return Err("graph-node-budget-exhausted".to_owned());
        }
        ranked.push((distance, seed_rank, node_id.clone()));
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
                    frontier.push_back((edge.target.clone(), distance + 1, seed_rank));
                    work.frontier_peak = work.frontier_peak.max(frontier.len());
                }
            }
        }
    }
    ranked.sort();
    ranked.truncate(budget.max_results);
    let ranked_nodes = ranked
        .into_iter()
        .map(|(distance, seed_rank, id)| {
            let node = &request.generation_graph.nodes_by_id[&id];
            ResidentGraphRankedNode {
                id,
                kind: node.kind.clone(),
                owner_path: node.owner_path.clone(),
                score: budget
                    .max_depth
                    .saturating_sub(distance)
                    .saturating_mul(1024)
                    .saturating_add(1024usize.saturating_sub(seed_rank)),
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
    nodes: &mut BTreeMap<String, Value>,
    endpoint: &ProviderProjectedRelationEndpoint,
) -> String {
    if endpoint.kind == "owner" {
        return insert_owner_node(nodes, &endpoint.id);
    }
    let node_id = stable_graph_node_id(&endpoint.kind, &endpoint.id);
    nodes.entry(node_id.clone()).or_insert_with(|| {
        let action = match endpoint.kind.as_str() {
            "owner" => "owner",
            "item" => "code",
            "test" => "tests",
            _ => "topology",
        };
        json!({
            "id": node_id,
            "kind": endpoint.kind,
            "role": endpoint.kind,
            "value": endpoint.id,
            "action": action,
            "confidence": "parser",
        })
    });
    node_id
}

fn insert_owner_node(nodes: &mut BTreeMap<String, Value>, owner_path: &str) -> String {
    let node_id = stable_graph_node_id("owner", owner_path);
    nodes.entry(node_id.clone()).or_insert_with(|| {
        json!({
            "id": node_id,
            "kind": "owner",
            "role": "path",
            "value": owner_path,
            "action": "owner",
            "path": owner_path,
            "ownerPath": owner_path,
            "confidence": "exact",
        })
    });
    node_id
}
