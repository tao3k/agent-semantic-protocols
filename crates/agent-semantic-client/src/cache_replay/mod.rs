//! Replay cache artifacts into compact prompt stdout.

mod artifact;
mod syntax_query;

pub(crate) use artifact::{
    MAX_CACHE_REPLAY_ARTIFACT_BYTES, ProviderCacheReplay, load_replay_artifact,
    replay_artifact_path,
};
#[cfg(test)]
pub(crate) use artifact::{
    query_packet_matches_request, semantic_tree_sitter_query_packet_matches_request,
};
#[cfg(test)]
pub(crate) use syntax_query::{
    render_semantic_tree_sitter_query_rows_stdout, render_semantic_tree_sitter_query_stdout,
};
