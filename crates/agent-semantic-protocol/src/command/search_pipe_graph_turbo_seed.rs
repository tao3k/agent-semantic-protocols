//! Query-owner seed selection facade for graph-turbo request packets.

use super::search_pipe_model::Candidate;

pub(super) fn has_package_path_candidate(candidates: &[Candidate], query_terms: &[String]) -> bool {
    let projection_candidates = graph_projection_candidates(candidates);
    agent_semantic_search::graph_has_package_path_candidate(&projection_candidates, query_terms)
}

pub(super) fn query_owner_seed_paths(
    candidates: &[Candidate],
    owners: &[String],
    budget: usize,
    query_terms: &[String],
) -> Vec<String> {
    let projection_candidates = graph_projection_candidates(candidates);
    agent_semantic_search::graph_query_owner_seed_paths(
        &projection_candidates,
        owners,
        budget,
        query_terms,
    )
}

fn graph_projection_candidates(
    candidates: &[Candidate],
) -> Vec<agent_semantic_search::GraphProjectionCandidate> {
    candidates
        .iter()
        .map(|candidate| {
            agent_semantic_search::GraphProjectionCandidate::new(
                candidate.path.clone(),
                candidate.line,
                candidate.end_line,
                candidate.symbol.clone(),
                candidate.text.clone(),
                candidate.source.clone(),
                candidate.confidence.clone(),
            )
        })
        .collect()
}
