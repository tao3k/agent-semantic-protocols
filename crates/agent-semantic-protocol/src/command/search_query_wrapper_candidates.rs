//! Candidate collection and finder previews for query wrappers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_semantic_client::{SourceIndexLookupResult, lookup_source_index_for_language};
use agent_semantic_search::{
    QueryWrapperSearchClause, QueryWrapperSearchRequest, QueryWrapperSearchSourceIndexTrace,
    QueryWrapperSearchSurface, QueryWrapperSourceIndexCandidate, QueryWrapperSourceIndexLookup,
    collect_query_wrapper_candidate_collection as collect_search_query_wrapper_candidate_collection,
    query_wrapper_axis_terms, query_wrapper_terms,
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
    let status = query_wrapper_source_index_status(&trace.lookup.state);
    let mut fields = BTreeMap::new();
    fields.insert(
        "collectMs".to_string(),
        Value::from(trace.elapsed.as_millis().min(u128::from(u64::MAX)) as u64),
    );
    fields.insert("state".to_string(), Value::from(trace.lookup.state.clone()));
    fields.insert(
        "dbPath".to_string(),
        Value::from(trace.lookup.db_path.display().to_string()),
    );
    if status != "used" {
        fields.insert(
            "nextCommand".to_string(),
            Value::from("asp cache source-index refresh"),
        );
    }
    SearchPipeSourceTrace::new(
        "sourceIndex",
        status,
        trace.candidate_count,
        usize::from(trace.candidate_count == 0),
        trace.candidate_count,
    )
    .with_fields(fields)
}

fn query_wrapper_source_index_status(state: &str) -> &'static str {
    match state {
        "hit" => "used",
        "missing-db" => "missing-db",
        "empty-index" => "empty-index",
        "miss" => "miss",
        _ => "unknown",
    }
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
    QueryWrapperSourceIndexLookup {
        db_path: result.db_path,
        state: result.state.as_str().to_string(),
        candidates: result
            .candidates
            .into_iter()
            .map(|candidate| QueryWrapperSourceIndexCandidate {
                path: candidate.path,
                language_id: candidate
                    .language_id
                    .map(|value| value.as_str().to_string()),
                provider_id: candidate
                    .provider_id
                    .map(|value| value.as_str().to_string()),
                source_kind: candidate.source_kind.as_str().to_string(),
                line_count: candidate.line_count,
                query_keys: candidate.query_keys,
            })
            .collect(),
    }
}

fn query_wrapper_search_surface(surface: QueryWrapperSurface) -> QueryWrapperSearchSurface {
    match surface {
        QueryWrapperSurface::Fd => QueryWrapperSearchSurface::Fd,
        QueryWrapperSurface::Rg => QueryWrapperSearchSurface::Rg,
    }
}

pub(super) fn query_clauses(queries: &[String]) -> Vec<QueryWrapperClause> {
    queries
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let terms = query_terms(raw);
            (!terms.is_empty()).then_some(QueryWrapperClause {
                id: index + 1,
                raw: raw.clone(),
                terms,
                axis_terms: query_wrapper_axis_terms(raw),
            })
        })
        .collect()
}

pub(super) fn unique_clause_terms(clauses: &[QueryWrapperClause]) -> Vec<String> {
    clauses
        .iter()
        .flat_map(|clause| clause.terms.iter())
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == term) {
                terms.push(term.clone());
            }
            terms
        })
}

fn query_terms(query: &str) -> Vec<String> {
    query_wrapper_terms(query)
}

pub(super) fn owner_candidates(candidates: &[Candidate]) -> Vec<String> {
    unique_take(candidates.iter().map(|candidate| candidate.path.clone()), 8)
}

pub(super) fn package_clusters(candidates: &[Candidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .map(|candidate| package_key(&candidate.path)),
        6,
    )
}

pub(super) fn rg_scope_next(candidates: &[Candidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .map(|candidate| package_key(&candidate.path))
            .filter(|package| !package.is_empty()),
        3,
    )
}

pub(super) fn package_key(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        let end = (index + 3).min(parts.len());
        return parts[index..end].join("/");
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .take(2)
        .collect::<Vec<_>>()
        .join("/")
}

fn unique_take(values: impl Iterator<Item = String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

pub(super) fn absolute_scope(root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    }
}
