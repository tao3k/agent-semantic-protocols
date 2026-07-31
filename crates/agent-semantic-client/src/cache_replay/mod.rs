//! Replay cache artifacts into compact prompt stdout.

#[cfg(test)]
pub(crate) use agent_semantic_client_core::structured_evidence_artifact_path;

#[cfg(test)]
pub(crate) use agent_semantic_search::output_with_delegation_hint_lines;

mod artifact;
mod limits;
mod search_lexical;
mod search_packet;
mod syntax_query;

pub(crate) use agent_semantic_client_core::replay_artifact_path;
pub(crate) use agent_semantic_search::search_output_artifact_replay_safe;
pub(crate) use artifact::{ProviderCacheReplay, load_replay_artifact};
pub(crate) use limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;
pub(crate) use search_lexical::search_lexical_generation_matches_request;
pub(crate) use search_packet::{
    SearchFrontierReceiptRequest, render_search_packet_bytes,
    render_search_packet_bytes_with_receipt,
};
