//! Compatibility facade for source-index candidate lookup.

use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexSourceKind};
use agent_semantic_search::{
    QUERY_WRAPPER_CANDIDATE_LIMIT, QueryWrapperSearchSurface, QueryWrapperSourceIndexCandidate,
    QueryWrapperSourceIndexLookup, SearchPipeSourceIndexCandidate, SearchPipeSourceIndexLookup,
};

pub use agent_semantic_search::{
    SourceIndexClientCacheLookupRequest, SourceIndexLookupRequest, lookup_source_index,
    lookup_source_index_for_language, lookup_source_index_in_cache,
    lookup_source_index_in_client_cache_dir,
};

/// Lookup and project source-index rows for query-wrapper candidate collection.
pub fn lookup_query_wrapper_source_index(
    surface: QueryWrapperSearchSurface,
    project_root: &Path,
    terms: &[String],
) -> Result<Option<QueryWrapperSourceIndexLookup>, String> {
    if terms.is_empty() {
        return Ok(None);
    }
    let query = terms.join(" ");
    let result = lookup_source_index_for_language(
        project_root,
        None,
        query.as_str(),
        query_wrapper_source_index_limit(surface),
    )?;
    Ok(Some(query_wrapper_source_index_lookup_from_client_result(
        result,
    )))
}

/// Lookup stable source-index owner candidates for search-pipe source acquisition.
pub fn lookup_search_pipe_source_index_for_language(
    project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<SearchPipeSourceIndexLookup, String> {
    let result = lookup_source_index_for_language(project_root, language_id, query, limit)?;
    Ok(search_pipe_source_index_lookup_from_client_result(result))
}

fn query_wrapper_source_index_limit(surface: QueryWrapperSearchSurface) -> u32 {
    match surface {
        QueryWrapperSearchSurface::Fd => 16,
        QueryWrapperSearchSurface::Rg => QUERY_WRAPPER_CANDIDATE_LIMIT as u32,
    }
}

pub(crate) fn search_pipe_source_index_lookup_from_client_result(
    result: ClientDbSourceIndexLookupResult,
) -> SearchPipeSourceIndexLookup {
    SearchPipeSourceIndexLookup {
        state: result.state.as_str().to_string(),
        candidates: result
            .candidates
            .into_iter()
            .map(|candidate| SearchPipeSourceIndexCandidate {
                path: candidate.path,
                language_id: candidate
                    .language_id
                    .map(|value| value.as_str().to_string()),
                provider_id: candidate
                    .provider_id
                    .map(|value| value.as_str().to_string()),
                source_kind: source_index_candidate_kind(&candidate.source_kind).to_string(),
                line_count: candidate.line_count,
                query_keys: candidate.query_keys,
                selector_proof: candidate.selector_proof.map(|proof| {
                    agent_semantic_search::SearchPipeSelectorPayloadProof {
                        structural_selector: proof.structural_selector,
                        payload_kind: proof.payload_kind,
                        bounded: proof.bounded,
                    }
                }),
            })
            .collect(),
    }
}

pub(crate) fn query_wrapper_source_index_lookup_from_client_result(
    result: ClientDbSourceIndexLookupResult,
) -> QueryWrapperSourceIndexLookup {
    QueryWrapperSourceIndexLookup::new(
        result.db_path,
        result.state.as_str(),
        result
            .candidates
            .into_iter()
            .map(|candidate| {
                QueryWrapperSourceIndexCandidate::new(
                    (
                        candidate.path,
                        candidate
                            .language_id
                            .map(|value| value.as_str().to_string()),
                        candidate
                            .provider_id
                            .map(|value| value.as_str().to_string()),
                        source_index_candidate_kind(&candidate.source_kind),
                        candidate.line_count,
                        candidate.query_keys,
                    )
                        .into(),
                )
            })
            .collect(),
    )
}

fn source_index_candidate_kind(kind: &ClientDbSourceIndexSourceKind) -> &str {
    kind.as_str()
}
