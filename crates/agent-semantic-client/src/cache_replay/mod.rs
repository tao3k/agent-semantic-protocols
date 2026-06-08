//! Replay cache artifacts into compact prompt stdout.

mod artifact;
mod graph_render;
mod limits;
mod search_fzf;
mod search_packet;
mod syntax_query;

pub(crate) use artifact::{
    ProviderCacheReplay, load_replay_artifact, load_syntax_query_rows_replay,
    render_query_packet_bytes, replay_artifact_path,
};
#[cfg(test)]
pub(crate) use artifact::{
    query_packet_matches_request, semantic_tree_sitter_query_packet_matches_request,
    structured_evidence_artifact_path,
};
pub(crate) use limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;
pub(crate) use search_fzf::search_fzf_generation_matches_request;
#[cfg(test)]
pub(crate) use search_fzf::search_fzf_packet_matches_request;
pub(crate) use search_packet::{
    SearchFrontierReceiptRequest, render_search_packet_bytes,
    render_search_packet_bytes_with_receipt, search_output_artifact_replay_safe,
};
#[cfg(test)]
pub(crate) use syntax_query::{
    render_semantic_tree_sitter_query_rows_stdout, render_semantic_tree_sitter_query_stdout,
};
