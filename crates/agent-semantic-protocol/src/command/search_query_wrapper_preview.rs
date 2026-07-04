//! Query-overlay preview helpers for query wrappers and search pipe plans.

use super::search_pipe_model::Candidate;
use super::search_query_wrapper_candidates::{owner_candidates, package_clusters, rg_scope_next};
use super::search_query_wrapper_model::FdQueryPreview;

pub(super) fn fd_query_preview_from_candidates(candidates: &[Candidate]) -> Option<FdQueryPreview> {
    let preview = FdQueryPreview {
        owner_candidates: owner_candidates(candidates).into_iter().take(4).collect(),
        package_clusters: package_clusters(candidates).into_iter().take(1).collect(),
        rg_scope_next: rg_scope_next(candidates).into_iter().take(1).collect(),
    };
    (!preview.is_empty()).then_some(preview)
}
