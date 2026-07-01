//! Replay cache artifacts into compact prompt stdout.

mod artifact;
mod limits;
mod search_lexical;
mod search_packet;
mod syntax_query;

pub(crate) use agent_semantic_client_core::replay_artifact_path;
#[cfg(test)]
pub(crate) use agent_semantic_client_core::structured_evidence_artifact_path;
#[cfg(test)]
pub(crate) use agent_semantic_search::output_with_delegation_hint_lines;
pub(crate) use agent_semantic_search::search_output_artifact_replay_safe;
pub(crate) use artifact::{
    ProviderCacheReplay, load_replay_artifact, load_syntax_query_rows_replay,
    load_syntax_query_rows_replay_session, render_query_packet_bytes,
};
#[cfg(test)]
pub(crate) use artifact::{
    query_packet_matches_request, semantic_tree_sitter_query_packet_matches_request,
};
pub(crate) use limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;
pub(crate) use search_lexical::search_lexical_generation_matches_request;
#[cfg(test)]
pub(crate) use search_lexical::search_lexical_packet_matches_request;
pub(crate) use search_packet::{
    SearchFrontierReceiptRequest, render_search_packet_bytes,
    render_search_packet_bytes_with_receipt,
};
#[cfg(test)]
pub(crate) use syntax_query::{
    render_semantic_tree_sitter_query_rows_stdout, render_semantic_tree_sitter_query_stdout,
};
