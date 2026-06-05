//! Receipt annotations for syntax-query provider packets.

use agent_semantic_client_core::{ByteCount, CacheArtifactId, ClientReceipt};
use serde_json::Value;

const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

pub(crate) fn apply_syntax_query_receipt_metadata(receipt: &mut ClientReceipt, stdout: &[u8]) {
    let Ok(packet) = serde_json::from_slice::<Value>(stdout) else {
        return;
    };
    if packet.get("schemaId").and_then(Value::as_str) != Some(SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID)
    {
        return;
    }

    receipt.packet_bytes = Some(ByteCount::from_len(stdout.len()));
    receipt.syntax_artifact_id = packet
        .pointer("/cache/artifactId")
        .and_then(Value::as_str)
        .filter(|artifact_id| !artifact_id.is_empty())
        .map(CacheArtifactId::from);
}
