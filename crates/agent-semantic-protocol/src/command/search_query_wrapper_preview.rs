//! Finder preview helpers for query wrappers and search pipe plans.

use std::path::{Path, PathBuf};

use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;
use super::search_query_wrapper_candidates::{
    QueryCandidateRequest, collect_query_candidates, owner_candidates, package_clusters,
    query_clauses, rg_scope_next, unique_clause_terms,
};
use super::search_query_wrapper_model::{FdQueryPreview, QueryWrapperSurface};

pub(super) fn fd_query_preview(
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    query: &str,
) -> Option<FdQueryPreview> {
    let config = AspConfig::load(locator_root, project_root);
    let queries = vec![query.to_string()];
    let clauses = query_clauses(&queries);
    let terms = unique_clause_terms(&clauses);
    let preview_scopes = scopes
        .iter()
        .map(|scope| {
            if scope.is_absolute() {
                scope.clone()
            } else {
                project_root.join(scope)
            }
        })
        .collect::<Vec<_>>();
    let candidates = collect_query_candidates(QueryCandidateRequest {
        surface: QueryWrapperSurface::Fd,
        project_root,
        locator_root: project_root,
        scopes: &preview_scopes,
        clauses: &clauses,
        terms: &terms,
        config: &config,
        native_args: &[],
    })
    .ok()?;
    fd_query_preview_from_candidates(&candidates)
}

pub(super) fn fd_query_preview_from_candidates(candidates: &[Candidate]) -> Option<FdQueryPreview> {
    let preview = FdQueryPreview {
        owner_candidates: owner_candidates(candidates).into_iter().take(4).collect(),
        package_clusters: package_clusters(candidates).into_iter().take(1).collect(),
        rg_scope_next: rg_scope_next(candidates).into_iter().take(1).collect(),
    };
    (!preview.is_empty()).then_some(preview)
}
