//! Candidate collection and query-overlay previews for query wrappers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_semantic_client::lookup_query_wrapper_source_index;
use agent_semantic_search::{
    QUERY_OVERLAY_ROUTE_SOURCE, QueryWrapperSearchClause, QueryWrapperSearchRequest,
    QueryWrapperSearchSourceIndexTrace, QueryWrapperSearchStageTraceProjection,
    QueryWrapperSearchSurface,
    collect_query_wrapper_candidate_collection as collect_search_query_wrapper_candidate_collection,
    query_wrapper_clauses as search_query_wrapper_clauses, query_wrapper_owner_candidates,
    query_wrapper_package_clusters_from_paths, query_wrapper_rg_scope_next,
    query_wrapper_source_index_trace_projection,
    query_wrapper_unique_clause_terms as search_query_wrapper_unique_clause_terms,
};
use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};
use super::search_query_wrapper_model::{QueryWrapperClause, QueryWrapperSurface};

pub(super) struct QueryCandidateCollection {
    pub(super) candidates: Vec<Candidate>,
    pub(super) trace_fields: BTreeMap<String, Value>,
    pub(super) source_trace: Vec<SearchPipeSourceTrace>,
    pub(super) candidate_sources: Vec<String>,
}

pub(super) struct QueryCandidateRequest<'a> {
    pub(super) surface: QueryWrapperSurface,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) scopes: &'a [PathBuf],
    pub(super) clauses: &'a [QueryWrapperClause],
    pub(super) terms: &'a [String],
    pub(super) config: &'a AspConfig,
    pub(super) native_args: &'a [String],
}

impl QueryCandidateCollection {
    pub(super) fn blocked(gate: &super::search_query_budget::SearchQueryBudgetBlock) -> Self {
        let mut trace_fields = BTreeMap::new();
        trace_fields.insert("blocked".to_string(), Value::from(true));
        trace_fields.insert("reason".to_string(), Value::from(gate.reason));
        trace_fields.insert("termCount".to_string(), Value::from(gate.term_count));
        trace_fields.insert(
            "genericTerms".to_string(),
            Value::from(gate.generic_terms.join("|")),
        );
        Self {
            candidates: Vec::new(),
            trace_fields,
            source_trace: Vec::new(),
            candidate_sources: vec![QUERY_OVERLAY_ROUTE_SOURCE.to_string()],
        }
    }
}

pub(super) fn collect_query_candidate_collection(
    request: QueryCandidateRequest<'_>,
) -> Result<QueryCandidateCollection, String> {
    let QueryCandidateRequest {
        surface,
        project_root,
        locator_root,
        scopes,
        clauses,
        terms,
        config,
        native_args,
    } = request;
    let search_clauses = clauses
        .iter()
        .map(|clause| QueryWrapperSearchClause {
            id: clause.id,
            raw: clause.raw.clone(),
            terms: clause.terms.clone(),
            axis_terms: clause.axis_terms.clone(),
        })
        .collect::<Vec<_>>();
    let search_surface = query_wrapper_search_surface(surface);
    let source_index_lookup =
        lookup_query_wrapper_source_index(search_surface, project_root, terms)?;
    let ranked_search_candidates = Vec::new();
    let collection =
        collect_search_query_wrapper_candidate_collection(QueryWrapperSearchRequest {
            surface: search_surface,
            project_root,
            locator_root,
            scopes,
            clauses: &search_clauses,
            terms,
            ignore_dirs: &config.search.ignore_dirs,
            include_hidden_dirs: &config.search.include_hidden_dirs,
            native_args,
            ranked_search_candidates: &ranked_search_candidates,
            source_index_lookup,
        })?;
    let mut source_trace = Vec::new();
    let search_stage_trace_empty = collection.search_stage_trace_projections.is_empty();
    for projection in collection.search_stage_trace_projections {
        source_trace.push(query_wrapper_search_stage_trace(projection));
    }
    if let Some(trace) = collection.source_index_trace {
        source_trace.push(query_wrapper_source_index_trace(trace));
        if search_stage_trace_empty {
            let query_overlay_matched = collection.candidates.len();
            source_trace.push(SearchPipeSourceTrace::new(
                QUERY_OVERLAY_ROUTE_SOURCE,
                if collection.query_overlay_skipped_after_source_index {
                    "skipped"
                } else if query_overlay_matched == 0 {
                    "empty"
                } else {
                    "used"
                },
                query_overlay_matched,
                usize::from(query_overlay_matched == 0),
                query_overlay_matched,
            ));
        }
    }
    Ok(QueryCandidateCollection {
        candidates: collection
            .candidates
            .into_iter()
            .map(Candidate::from)
            .collect(),
        trace_fields: collection.trace_fields,
        source_trace,
        candidate_sources: collection.candidate_sources,
    })
}

fn query_wrapper_source_index_trace(
    trace: QueryWrapperSearchSourceIndexTrace,
) -> SearchPipeSourceTrace {
    let projection = query_wrapper_source_index_trace_projection(&trace);
    SearchPipeSourceTrace::new(
        projection.source,
        projection.status,
        projection.candidate_count,
        projection.skipped_count,
        projection.input_count,
    )
    .with_fields(projection.fields)
}

fn query_wrapper_search_stage_trace(
    projection: QueryWrapperSearchStageTraceProjection,
) -> SearchPipeSourceTrace {
    SearchPipeSourceTrace::new(
        projection.source,
        projection.status,
        projection.candidate_count,
        projection.skipped_count,
        projection.input_count,
    )
    .with_fields(projection.fields)
}

fn query_wrapper_search_surface(surface: QueryWrapperSurface) -> QueryWrapperSearchSurface {
    match surface {
        QueryWrapperSurface::Fd => QueryWrapperSearchSurface::Fd,
        QueryWrapperSurface::Rg => QueryWrapperSearchSurface::Rg,
    }
}

pub(super) fn query_clauses(queries: &[String]) -> Vec<QueryWrapperClause> {
    search_query_wrapper_clauses(queries)
        .into_iter()
        .map(|clause| QueryWrapperClause {
            id: clause.id,
            raw: clause.raw,
            terms: clause.terms,
            axis_terms: clause.axis_terms,
        })
        .collect()
}

pub(super) fn unique_clause_terms(clauses: &[QueryWrapperClause]) -> Vec<String> {
    let search_clauses = clauses
        .iter()
        .map(|clause| QueryWrapperSearchClause {
            id: clause.id,
            raw: clause.raw.clone(),
            terms: clause.terms.clone(),
            axis_terms: clause.axis_terms.clone(),
        })
        .collect::<Vec<_>>();
    search_query_wrapper_unique_clause_terms(&search_clauses)
}

pub(super) fn owner_candidates(candidates: &[Candidate]) -> Vec<String> {
    query_wrapper_owner_candidates(candidates.iter().map(|candidate| candidate.path.clone()))
}

pub(super) fn package_clusters(candidates: &[Candidate]) -> Vec<String> {
    query_wrapper_package_clusters_from_paths(
        candidates.iter().map(|candidate| candidate.path.clone()),
    )
}

pub(super) fn rg_scope_next(candidates: &[Candidate]) -> Vec<String> {
    query_wrapper_rg_scope_next(candidates.iter().map(|candidate| candidate.path.clone()))
}

pub(super) fn absolute_scope(root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    }
}
