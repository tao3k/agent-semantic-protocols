use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientReceipt, ElapsedMillis, LanguageId, NativeProvenance,
    ProviderCommandReceipt, ProviderId,
};

use crate::syntax_receipt::apply_syntax_query_receipt_metadata;

fn base_receipt() -> ClientReceipt {
    ClientReceipt::local_native(
        ClientMethod::Query,
        NativeProvenance {
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            provider_binary: "rs-harness".to_string(),
        },
        ProviderCommandReceipt {
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            argv: vec!["rs-harness".to_string(), "query".to_string()],
            exit_code: 0,
            stdout_bytes: ByteCount::new(0),
            stderr_bytes: ByteCount::new(0),
            elapsed_ms: ElapsedMillis::new(0),
        },
    )
}

#[test]
fn annotates_semantic_tree_sitter_query_packet_receipt() {
    let stdout = br#"{"schemaId":"agent.semantic-protocols.semantic-tree-sitter-query","cache":{"artifactId":"semantic-tree-sitter-query/calls.json"}}"#;
    let mut receipt = base_receipt();

    apply_syntax_query_receipt_metadata(&mut receipt, stdout);

    assert_eq!(
        receipt.packet_bytes,
        Some(ByteCount::from_len(stdout.len()))
    );
    assert_eq!(
        receipt.syntax_artifact_id.as_ref().map(|id| id.as_str()),
        Some("semantic-tree-sitter-query/calls.json")
    );
}

#[test]
fn ignores_non_syntax_or_invalid_stdout() {
    let mut receipt = base_receipt();
    apply_syntax_query_receipt_metadata(&mut receipt, br#"{"schemaId":"other"}"#);
    apply_syntax_query_receipt_metadata(&mut receipt, b"not json");

    assert_eq!(receipt.packet_bytes, None);
    assert_eq!(receipt.syntax_artifact_id, None);
}
