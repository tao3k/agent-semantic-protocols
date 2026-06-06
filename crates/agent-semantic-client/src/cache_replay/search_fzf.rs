//! Search fzf replay matching for cache artifact reuse.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::ClientMethod;
use agent_semantic_client_core::ClientRequest;
use agent_semantic_client_db::ClientDbGenerationHit;
use serde_json::Value;

use super::artifact::{MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path};

pub(crate) fn search_fzf_generation_matches_request(
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
        search_fzf_packet_matches_request(&packet, request)
    })
}

pub(crate) fn search_fzf_packet_matches_request(
    packet: &Value,
    request: &ClientRequest,
) -> Option<()> {
    if request.method != ClientMethod::Search
        || request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code")
    {
        return None;
    }
    if string_field(packet, "schemaId")? != "agent.semantic-protocols.semantic-search-packet" {
        return None;
    }
    if string_field(packet, "method")? != "search/fzf" {
        return None;
    }
    if string_field(packet, "query")? != request_search_fzf_query(&request.forwarded_args)? {
        return None;
    }
    Some(())
}

fn request_search_fzf_query(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args.windows(2).find_map(|window| {
        (window[0] == "fzf" && !window[1].starts_with('-') && window[1] != ".")
            .then_some(window[1].as_str())
    })
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}
