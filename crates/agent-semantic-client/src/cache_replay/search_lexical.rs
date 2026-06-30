//! Search lexical replay matching for cache artifact reuse.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::{ClientMethod, ClientRequest, replay_artifact_path};
use agent_semantic_client_db::ClientDbGenerationHit;
use agent_semantic_search::{
    SearchLexicalReplayRequest, search_lexical_packet_matches_request as search_packet_matches,
};
use serde_json::Value;

use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(crate) fn search_lexical_generation_matches_request(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<()> {
    generation_hit.artifact_ids.iter().find_map(|artifact_id| {
        let artifact_path = replay_artifact_path(cache_root, artifact_id, "search/", ".json")?;
        let metadata = fs::metadata(&artifact_path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
            return None;
        }
        let packet: Value = serde_json::from_slice(&fs::read(artifact_path).ok()?).ok()?;
        search_lexical_packet_matches_request(&packet, request)
    })
}

pub(crate) fn search_lexical_packet_matches_request(
    packet: &Value,
    request: &ClientRequest,
) -> Option<()> {
    search_packet_matches(
        packet,
        SearchLexicalReplayRequest {
            is_search_method: request.method == ClientMethod::Search,
            forwarded_args: &request.forwarded_args,
        },
    )
    .then_some(())
}
