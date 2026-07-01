//! Compatibility facade for source-index candidate lookup.

use std::path::Path;

use agent_semantic_client_db::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexSourceKind};
use agent_semantic_search::{
    QUERY_WRAPPER_CANDIDATE_LIMIT, QueryWrapperSearchSurface, QueryWrapperSourceIndexCandidate,
    QueryWrapperSourceIndexLookup,
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

fn query_wrapper_source_index_limit(surface: QueryWrapperSearchSurface) -> u32 {
    match surface {
        QueryWrapperSearchSurface::Fd => 16,
        QueryWrapperSearchSurface::Rg => QUERY_WRAPPER_CANDIDATE_LIMIT as u32,
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
            })
            .collect(),
    )
}

fn source_index_candidate_kind(kind: &ClientDbSourceIndexSourceKind) -> &str {
    kind.as_str()
}
