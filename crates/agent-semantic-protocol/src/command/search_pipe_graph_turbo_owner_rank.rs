//! Owner candidate ranking facade for graph-turbo seed construction.

use std::path::Path;

use super::search_pipe_model::Candidate;

pub(super) fn graph_owner_rank_report_with_topology(
    candidates: &[Candidate],
    query_terms: &[String],
    workspace_root: Option<&Path>,
) -> agent_semantic_search::GraphOwnerRankReport {
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
    let submodule_paths = workspace_root
        .map(agent_semantic_search::graph_project_submodule_paths)
        .unwrap_or_default();
    agent_semantic_search::rank_graph_owner_report(agent_semantic_search::GraphOwnerRankRequest {
        candidates: projection_candidates
            .iter()
            .map(agent_semantic_search::GraphOwnerRankCandidate::from)
            .collect(),
        query_terms: query_terms.to_vec(),
        submodule_paths,
    })
}
