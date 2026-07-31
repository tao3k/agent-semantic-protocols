//! Compatibility facade for source-index candidate lookup.

use agent_semantic_client_db::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexSourceKind};
use agent_semantic_search::{SearchPipeSourceIndexCandidate, SearchPipeSourceIndexLookup};

pub use agent_semantic_search::{
    SourceIndexLookupRequest, lookup_source_index, lookup_source_index_for_language,
    lookup_source_index_in_cache,
};

/// Lookup stable source-index owner candidates for search-pipe source acquisition.
pub fn lookup_search_pipe_source_index_for_language(
    request: SourceIndexLookupRequest<'_>,
) -> Result<SearchPipeSourceIndexLookup, String> {
    let result = lookup_source_index_in_cache(request)?;
    Ok(search_pipe_source_index_lookup_from_client_result(result))
}

pub(crate) fn search_pipe_source_index_lookup_from_client_result(
    result: ClientDbSourceIndexLookupResult,
) -> SearchPipeSourceIndexLookup {
    let source_snapshot = result.source_snapshot;
    let index_artifact_digest = result.index_artifact_digest;
    SearchPipeSourceIndexLookup {
        state: result.state.as_str().to_string().into(),
        candidates: result
            .candidates
            .into_iter()
            .map(|candidate| SearchPipeSourceIndexCandidate {
                path: candidate.path.as_str().to_string().into(),
                language_id: candidate
                    .language_id
                    .map(|value| value.as_str().to_string().into()),
                provider_id: candidate
                    .provider_id
                    .map(|value| value.as_str().to_string().into()),
                source_kind: source_index_candidate_kind(&candidate.source_kind)
                    .to_string()
                    .into(),
                line_count: candidate.line_count,
                query_keys: candidate
                    .query_keys
                    .into_iter()
                    .map(|key| key.as_str().to_string().into())
                    .collect(),
                selector_proof: candidate.selector_proof,
            })
            .collect(),
        source_snapshot,
        index_artifact_digest: index_artifact_digest.map(Into::into),
    }
}

fn source_index_candidate_kind(kind: &ClientDbSourceIndexSourceKind) -> &str {
    kind.as_str()
}
