//! Query-wrapper source-index and ranked-search candidate projections.

use std::path::{Path, PathBuf};

use crate::query_wrapper_scan::{
    QUERY_WRAPPER_CANDIDATE_LIMIT, QueryWrapperCandidate, QueryWrapperSearchCandidateCollection,
    QueryWrapperSearchCandidateRequest, QueryWrapperSourceIndexCandidate,
    QueryWrapperSourceIndexCollection, QueryWrapperSourceIndexRequest, query_candidate_priority,
};
use crate::search_candidate::{RankedSearchCandidate, SearchCandidate, SearchStageReceipt};

pub fn collect_query_wrapper_source_index_candidates(
    request: QueryWrapperSourceIndexRequest<'_>,
) -> Result<Option<QueryWrapperSourceIndexCollection>, String> {
    if !request.project_root.is_dir() {
        return Ok(None);
    }
    if matches!(
        request.lookup.state.as_str(),
        "missing-db" | "empty-index" | "busy"
    ) {
        return Ok(None);
    }
    let mut candidates = request
        .lookup
        .candidates
        .iter()
        .filter(|candidate| {
            source_index_candidate_in_roots(request.project_root, request.roots, &candidate.path)
        })
        .map(source_index_query_wrapper_candidate)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        query_candidate_priority(&candidate.path, request.terms, request.axis_terms)
    });
    Ok(Some(QueryWrapperSourceIndexCollection { candidates }))
}

pub fn collect_query_wrapper_search_candidates(
    request: QueryWrapperSearchCandidateRequest<'_>,
) -> Option<QueryWrapperSearchCandidateCollection> {
    let mut candidates = request
        .ranked
        .iter()
        .filter_map(|ranked| {
            search_candidate_query_wrapper_candidate(
                request.project_root,
                request.roots,
                &ranked.candidate,
            )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by_key(|candidate| {
        query_candidate_priority(&candidate.path, request.terms, request.axis_terms)
    });
    candidates.truncate(QUERY_WRAPPER_CANDIDATE_LIMIT);
    Some(QueryWrapperSearchCandidateCollection {
        stage_receipt: query_wrapper_search_stage_receipt(request.ranked, candidates.len()),
        candidates,
    })
}

fn query_wrapper_search_stage_receipt(
    ranked: &[RankedSearchCandidate],
    returned_count: usize,
) -> SearchStageReceipt {
    SearchStageReceipt {
        stage: "search-candidate-merge".to_string(),
        route_sources: query_wrapper_ranked_route_sources(ranked),
        candidate_count: ranked.len(),
        returned_count,
        filtered_line_identity_count: 0,
        fallback_reason: "none".to_string(),
    }
}

fn query_wrapper_ranked_route_sources(ranked: &[RankedSearchCandidate]) -> Vec<String> {
    let mut sources = ranked
        .iter()
        .map(|candidate| candidate.candidate.route_source.clone())
        .collect::<Vec<_>>();
    sources.sort();
    sources.dedup();
    sources
}

fn source_index_candidate_in_roots(project_root: &Path, roots: &[PathBuf], path: &str) -> bool {
    let candidate_path = Path::new(path);
    let candidate_abs = if candidate_path.is_absolute() {
        candidate_path.to_path_buf()
    } else {
        project_root.join(candidate_path)
    };
    roots.iter().any(|root| {
        if root.is_file() {
            candidate_abs == *root
        } else {
            candidate_abs.starts_with(root)
        }
    })
}

fn source_index_query_wrapper_candidate(
    candidate: &QueryWrapperSourceIndexCandidate,
) -> QueryWrapperCandidate {
    let line_count = candidate.line_count.unwrap_or(1).max(1) as usize;
    QueryWrapperCandidate {
        path: candidate.path.clone(),
        line: 1,
        end_line: line_count,
        symbol: source_index_symbol(candidate),
        selector: None,
        text: source_index_candidate_text(candidate),
        source: "source-index".to_string(),
        confidence: "db-engine".to_string(),
    }
}

fn search_candidate_query_wrapper_candidate(
    project_root: &Path,
    roots: &[PathBuf],
    candidate: &SearchCandidate,
) -> Option<QueryWrapperCandidate> {
    let path = search_candidate_path(candidate)?;
    if !source_index_candidate_in_roots(project_root, roots, &path) {
        return None;
    }
    Some(QueryWrapperCandidate {
        path,
        line: 1,
        end_line: 1,
        symbol: search_candidate_symbol(candidate),
        selector: candidate.selector.clone(),
        text: search_candidate_text(candidate),
        source: candidate.route_source.clone(),
        confidence: candidate.proof_source.clone(),
    })
}

fn search_candidate_path(candidate: &SearchCandidate) -> Option<String> {
    candidate
        .owner_path
        .clone()
        .or_else(|| candidate.selector.as_deref().and_then(selector_path_hint))
}

fn selector_path_hint(selector: &str) -> Option<String> {
    let path_with_fragment = selector
        .split_once("://")
        .map(|(_, path)| path)
        .unwrap_or(selector);
    path_with_fragment
        .split_once('#')
        .map(|(path, _)| path)
        .filter(|path| !path.is_empty())
        .map(ToString::to_string)
}

fn search_candidate_symbol(candidate: &SearchCandidate) -> String {
    candidate
        .selector
        .as_deref()
        .and_then(|selector| selector.split_once('#').map(|(_, fragment)| fragment))
        .and_then(|fragment| fragment.rsplit('/').next())
        .filter(|symbol| !symbol.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| candidate.candidate_id.clone())
}

fn search_candidate_text(candidate: &SearchCandidate) -> String {
    let fields = candidate
        .field_hits
        .iter()
        .map(|field| format!("{}={}", field.field, field.value))
        .collect::<Vec<_>>()
        .join(" ");
    let selector = candidate.selector.as_deref().unwrap_or("none");
    let generation = candidate.generation.as_deref().unwrap_or("none");
    format!(
        "search-candidate source={} fallback={} identity={} selector={} generation={} proof={} {}",
        candidate.route_source,
        candidate.fallback_reason,
        candidate.identity_kind,
        selector,
        generation,
        candidate.proof_source,
        fields
    )
}

fn source_index_symbol(candidate: &QueryWrapperSourceIndexCandidate) -> String {
    candidate
        .query_keys
        .first()
        .cloned()
        .unwrap_or_else(|| symbol_from_path(&candidate.path))
}

fn source_index_candidate_text(candidate: &QueryWrapperSourceIndexCandidate) -> String {
    let language = candidate.language_id.as_deref().unwrap_or("unknown");
    let provider = candidate.provider_id.as_deref().unwrap_or("unknown");
    let keys = candidate
        .query_keys
        .iter()
        .take(8)
        .cloned()
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "source-index path={} language={} provider={} kind={} queryKeys={}",
        candidate.path,
        language,
        provider,
        candidate.source_kind.as_str(),
        keys
    )
}

fn symbol_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("source")
        .to_string()
}
