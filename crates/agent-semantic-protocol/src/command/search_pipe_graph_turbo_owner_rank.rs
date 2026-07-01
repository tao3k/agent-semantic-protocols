//! Owner candidate ranking facade for graph-turbo seed construction.

use std::path::Path;

use super::search_pipe_model::Candidate;

pub(super) fn ranked_candidate_paths_with_topology(
    candidates: &[Candidate],
    query_terms: &[String],
    workspace_root: Option<&Path>,
) -> Vec<String> {
    let projection_candidates = candidates
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
        .collect::<Vec<_>>();
    agent_semantic_search::ranked_graph_owner_paths_with_topology(
        &projection_candidates,
        query_terms,
        workspace_root,
    )
}
