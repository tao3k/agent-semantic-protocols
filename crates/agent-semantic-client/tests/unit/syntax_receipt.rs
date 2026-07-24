use agent_semantic_client_core::{
    ByteCount, ClientMethod, ClientReceipt, ElapsedMillis, LanguageId, NativeProvenance,
    ProviderCommandReceipt, ProviderId, syntax_query_ast_abi_fingerprint,
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
            stdout_sha256: None,
            stderr_sha256: None,
            stdout_truncated: false,
            stderr_truncated: false,
            timed_out: false,
            exit_signal: None,
            memory_limit_bytes: None,
            memory_limit_enforced: false,
            memory_limit_exceeded: false,
            abnormal_termination: false,
            termination_reason: None,
            elapsed_ms: ElapsedMillis::new(0),
        },
    )
}

#[test]
fn annotates_semantic_tree_sitter_query_packet_receipt() {
    let stdout = br#"{"schemaId":"agent.semantic-protocols.semantic-tree-sitter-query","grammarId":"tree-sitter-rust","grammarProfileVersion":"1.0.0","query":{"input":"(function_item name: (identifier) @function.name)","compiledSource":"(function_item name: (identifier) @function.name)","fields":{"selector":"src/lib.rs:1:20"}},"cache":{"artifactId":"semantic-tree-sitter-query/calls.json"}}"#;
    let mut receipt = base_receipt();
    let expected_fingerprint =
        syntax_query_ast_abi_fingerprint("(function_item name: (identifier) @function.name)")
            .unwrap();

    apply_syntax_query_receipt_metadata(&mut receipt, stdout);

    assert_eq!(
        receipt.packet_bytes,
        Some(ByteCount::from_len(stdout.len()))
    );
    assert_eq!(
        receipt.syntax_artifact_id.as_ref().map(|id| id.as_str()),
        Some("semantic-tree-sitter-query/calls.json")
    );
    assert_eq!(
        receipt
            .syntax_query_ast_abi_fingerprint
            .as_ref()
            .map(|fingerprint| fingerprint.as_str()),
        Some(expected_fingerprint.as_str())
    );
    assert_eq!(
        receipt
            .syntax_query_grammar_id
            .as_ref()
            .map(|grammar_id| grammar_id.as_str()),
        Some("tree-sitter-rust")
    );
    assert_eq!(
        receipt
            .syntax_query_grammar_profile_version
            .as_ref()
            .map(|grammar_profile_version| grammar_profile_version.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        receipt
            .syntax_query_selector
            .as_ref()
            .map(|selector| selector.as_str()),
        Some("src/lib.rs:1:20")
    );
}

#[test]
fn ignores_non_syntax_or_invalid_stdout() {
    let mut receipt = base_receipt();
    apply_syntax_query_receipt_metadata(&mut receipt, br#"{"schemaId":"other"}"#);
    apply_syntax_query_receipt_metadata(&mut receipt, b"not json");

    assert_eq!(receipt.packet_bytes, None);
    assert_eq!(receipt.syntax_artifact_id, None);
    assert_eq!(receipt.syntax_query_ast_abi_fingerprint, None);
    assert_eq!(receipt.syntax_query_grammar_id, None);
    assert_eq!(receipt.syntax_query_grammar_profile_version, None);
    assert_eq!(receipt.syntax_query_selector, None);
}
