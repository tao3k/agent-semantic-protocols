//! Compatibility facade for source-index candidate lookup.

use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexSourceKind};
use agent_semantic_search::{SearchPipeSourceIndexCandidate, SearchPipeSourceIndexLookup};

pub use agent_semantic_search::{
    SourceIndexClientCacheLookupRequest, SourceIndexLookupRequest, lookup_source_index,
    lookup_source_index_for_language, lookup_source_index_in_cache,
    lookup_source_index_in_client_cache_dir,
};

/// Lookup stable source-index owner candidates for search-pipe source acquisition.
pub fn lookup_search_pipe_source_index_for_language(
    project_root: &Path,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<SearchPipeSourceIndexLookup, String> {
    let result =
        lookup_source_index_for_language(project_root, source_snapshot, language_id, query, limit)?;
    Ok(search_pipe_source_index_lookup_from_client_result(result))
}

pub(crate) fn search_pipe_source_index_lookup_from_client_result(
    result: ClientDbSourceIndexLookupResult,
) -> SearchPipeSourceIndexLookup {
    let source_snapshot = result.source_snapshot;
    let index_artifact_digest = result.index_artifact_digest;
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
        source_snapshot,
        index_artifact_digest,
    }
}

fn source_index_candidate_kind(kind: &ClientDbSourceIndexSourceKind) -> &str {
    kind.as_str()
}
