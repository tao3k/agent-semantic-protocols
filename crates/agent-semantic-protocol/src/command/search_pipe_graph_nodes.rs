//! Graph-turbo node projection for search-pipe candidates and project topology.

use std::path::Path;

use serde_json::Value;

use super::search_pipe_model::Candidate;

pub(super) fn append_project_topology_nodes(
    nodes: &mut Vec<Value>,
    edges: &mut Vec<Value>,
    language_id: &str,
    workspace_root: &Path,
    candidates: &[Candidate],
) {
    let projection_candidates = graph_projection_candidates(candidates);
    let projection = agent_semantic_search::graph_project_topology_projection(
        (
            language_id,
            workspace_root,
            projection_candidates.as_slice(),
        )
            .into(),
    );
    nodes.extend(projection.nodes);
    edges.extend(projection.edges);
}

pub(super) fn append_submodule_owner_edges(
    edges: &mut Vec<Value>,
    workspace_root: &Path,
    owners: &[String],
) {
    edges.extend(agent_semantic_search::graph_submodule_owner_edges(
        workspace_root,
        owners,
    ));
}

pub(super) fn append_candidate_nodes(
    nodes: &mut Vec<Value>,
    language_id: &str,
    candidates: &[Candidate],
    limit: usize,
) {
    let projection_candidates = graph_projection_candidates(candidates);
    nodes.extend(agent_semantic_search::graph_candidate_item_nodes(
        (language_id, projection_candidates.as_slice(), limit).into(),
    ));
}

pub(super) fn append_hot_nodes(
    nodes: &mut Vec<Value>,
    language_id: &str,
    candidates: &[Candidate],
    limit: usize,
) {
    let projection_candidates = graph_projection_candidates(candidates);
    nodes.extend(agent_semantic_search::graph_candidate_hot_nodes(
        (language_id, projection_candidates.as_slice(), limit).into(),
    ));
}

pub(super) fn candidate_node_id(candidate: &Candidate) -> String {
    agent_semantic_search::graph_candidate_item_node_id(&graph_projection_candidate(candidate))
}

pub(super) fn hot_node_id(candidate: &Candidate) -> String {
    agent_semantic_search::graph_candidate_hot_node_id(&graph_projection_candidate(candidate))
}

pub(super) fn stable_node_id(kind: &str, value: &str) -> String {
    agent_semantic_search::stable_graph_node_id(kind, value)
}

fn graph_projection_candidates(
    candidates: &[Candidate],
) -> Vec<agent_semantic_search::GraphProjectionCandidate> {
    candidates.iter().map(graph_projection_candidate).collect()
}

fn graph_projection_candidate(
    candidate: &Candidate,
) -> agent_semantic_search::GraphProjectionCandidate {
    agent_semantic_search::GraphProjectionCandidate::new(
        candidate.path.clone(),
        candidate.line,
        candidate.end_line,
        candidate.symbol.clone(),
        candidate.text.clone(),
        candidate.source.clone(),
        candidate.confidence.clone(),
    )
}
