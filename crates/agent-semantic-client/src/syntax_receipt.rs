//! Receipt annotations for syntax-query provider packets.

use agent_semantic_client_core::{
    ByteCount, CacheArtifactId, ClientReceipt, SyntaxQueryAstAbiFingerprint, SyntaxQueryGrammarId,
    SyntaxQueryGrammarProfileVersion, SyntaxQuerySelector, syntax_query_ast_abi_fingerprint,
};
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
    receipt.syntax_query_ast_abi_fingerprint = syntax_query_packet_source(&packet)
        .and_then(|source| syntax_query_ast_abi_fingerprint(source).ok())
        .map(SyntaxQueryAstAbiFingerprint::from);
    receipt.syntax_query_grammar_id = packet
        .get("grammarId")
        .and_then(Value::as_str)
        .filter(|grammar_id| !grammar_id.is_empty())
        .map(SyntaxQueryGrammarId::from);
    receipt.syntax_query_grammar_profile_version = packet
        .get("grammarProfileVersion")
        .and_then(Value::as_str)
        .filter(|grammar_profile_version| !grammar_profile_version.is_empty())
        .map(SyntaxQueryGrammarProfileVersion::from);
    receipt.syntax_query_selector = packet
        .pointer("/query/fields/selector")
        .and_then(Value::as_str)
        .filter(|selector| !selector.is_empty())
        .map(SyntaxQuerySelector::from);
}

fn syntax_query_packet_source(packet: &Value) -> Option<&str> {
    let query = packet.get("query")?;
    query
        .get("compiledSource")
        .and_then(Value::as_str)
        .or_else(|| query.get("input").and_then(Value::as_str))
}
