//! Packet validation and packet-owned cache facts for write-back.

use agent_semantic_client_core::{
    ClientCacheFileHash, ResolvedProvider, syntax_query_ast_abi_fingerprint,
};

pub(super) const SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-structural-index";
pub(super) const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

pub(super) fn validate_search_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-search-packet" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    let has_search_synthesis = packet
        .get("searchSynthesis")
        .and_then(|value| value.as_object())
        .is_some();
    let has_graph = packet
        .get("nodes")
        .and_then(|value| value.as_array())
        .is_some()
        && packet
            .get("edges")
            .and_then(|value| value.as_array())
            .is_some();
    let has_frontier_lists = packet
        .get("owners")
        .and_then(|value| value.as_array())
        .is_some()
        || packet
            .get("hits")
            .and_then(|value| value.as_array())
            .is_some();
    if !has_search_synthesis && !has_graph && !has_frontier_lists {
        return None;
    }
    Some(())
}

pub(super) fn validate_query_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-query-packet" {
        return None;
    }
    if packet.get("method")?.as_str()? != "query/owner-items" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    packet.get("matches")?.as_array()?;
    Some(())
}

pub(super) fn validate_syntax_query_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    packet.get("grammarId")?.as_str()?;
    packet.get("grammarProfileVersion")?.as_str()?;
    let query_source = syntax_query_packet_source(&packet)?;
    syntax_query_ast_abi_fingerprint(query_source).ok()?;
    packet.get("query")?.as_object()?;
    packet.get("matches")?.as_array()?;
    if packet
        .pointer("/cache/artifactKind")
        .and_then(serde_json::Value::as_str)
        != Some("semantic-tree-sitter-query")
    {
        return None;
    }
    Some(())
}

pub(super) fn validate_structural_index_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID {
        return None;
    }
    if packet.get("schemaVersion")?.as_str()? != "1" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    if packet
        .get("rawSourceStored")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        return None;
    }
    if structural_index_file_hashes(&packet)?.is_empty() {
        return None;
    }
    packet.get("owners")?.as_array()?;
    packet.get("symbols")?.as_array()?;
    packet.get("dependencyUsages")?.as_array()?;
    Some(())
}

pub(super) fn packet_file_hashes(packet_bytes: &[u8]) -> Option<Vec<ClientCacheFileHash>> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let hashes = packet.pointer("/cache/fileHashes")?.as_array()?;
    parse_file_hashes(hashes)
}

pub(super) fn structural_index_file_hashes(
    packet: &serde_json::Value,
) -> Option<Vec<ClientCacheFileHash>> {
    let hashes = packet.get("fileHashes")?.as_array()?;
    let file_hashes = hashes
        .iter()
        .map(|hash| {
            Some(ClientCacheFileHash {
                path: hash.get("path")?.as_str()?.to_string(),
                sha256: hash.get("sha256")?.as_str()?.to_string(),
                byte_len: hash
                    .get("byteLen")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                mtime_ms: hash
                    .get("mtimeMs")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

pub(super) fn syntax_query_packet_source(packet: &serde_json::Value) -> Option<&str> {
    let query = packet.get("query")?;
    query
        .get("compiledSource")
        .and_then(serde_json::Value::as_str)
        .or_else(|| query.get("input").and_then(serde_json::Value::as_str))
}

fn parse_file_hashes(hashes: &[serde_json::Value]) -> Option<Vec<ClientCacheFileHash>> {
    let file_hashes = hashes
        .iter()
        .map(|hash| {
            Some(ClientCacheFileHash {
                path: hash.get("path")?.as_str()?.to_string(),
                sha256: hash.get("sha256")?.as_str()?.to_string(),
                byte_len: hash.get("byteLen")?.as_u64()?,
                mtime_ms: hash.get("mtimeMs")?.as_u64()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}
