//! Candidate collection and finder previews for query wrappers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_semantic_client::{SourceIndexLookupResult, lookup_source_index_for_language};
use agent_semantic_search::{
    QueryWrapperSearchClause, QueryWrapperSearchRequest, QueryWrapperSearchSourceIndexTrace,
    QueryWrapperSearchSurface, QueryWrapperSourceIndexCandidate, QueryWrapperSourceIndexLookup,
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
            candidate_sources: vec!["finder".to_string()],
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
    let collection =
        collect_search_query_wrapper_candidate_collection(QueryWrapperSearchRequest {
            surface: query_wrapper_search_surface(surface),
            project_root,
            locator_root,
            scopes,
            clauses: &search_clauses,
            terms,
            ignore_dirs: &config.search.ignore_dirs,
            include_hidden_dirs: &config.search.include_hidden_dirs,
            native_args,
            source_index_lookup: query_wrapper_source_index_lookup(surface, project_root, terms)?,
        })?;
    let mut source_trace = Vec::new();
    if let Some(trace) = collection.source_index_trace {
        source_trace.push(query_wrapper_source_index_trace(trace));
        if collection.finder_skipped_after_source_index {
            source_trace.push(SearchPipeSourceTrace::new("finder", "skipped", 0, 0, 0));
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

fn query_wrapper_source_index_lookup(
    surface: QueryWrapperSurface,
    project_root: &Path,
    terms: &[String],
) -> Result<Option<QueryWrapperSourceIndexLookup>, String> {
    if terms.is_empty() {
        return Ok(None);
    }
    let query = terms.join(" ");
    let limit = match surface {
        QueryWrapperSurface::Fd => 16,
        QueryWrapperSurface::Rg => agent_semantic_search::QUERY_WRAPPER_CANDIDATE_LIMIT as u32,
    };
    let result = lookup_source_index_for_language(project_root, None, &query, limit)?;
    Ok(Some(query_wrapper_source_index_lookup_from_client(result)))
}

fn query_wrapper_source_index_lookup_from_client(
    result: SourceIndexLookupResult,
) -> QueryWrapperSourceIndexLookup {
    QueryWrapperSourceIndexLookup::new(
        result.db_path,
        result.state.as_str(),
        result
            .candidates
            .into_iter()
            .map(|candidate| {
                QueryWrapperSourceIndexCandidate::new(
                    candidate.path,
                    candidate
                        .language_id
                        .map(|value| value.as_str().to_string()),
                    candidate
                        .provider_id
                        .map(|value| value.as_str().to_string()),
                    candidate.source_kind.as_str(),
                    candidate.line_count,
                    candidate.query_keys,
                )
            })
            .collect(),
    )
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
