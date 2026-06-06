use super::syntax_packet::{expected_frontier_stdout, expected_miss_stdout};
use crate::cache_replay::render_semantic_tree_sitter_query_rows_stdout;
use agent_semantic_client_core::{CacheArtifactId, CacheGenerationId, LanguageId};
use agent_semantic_client_db::{
    ClientDbSyntaxCaptureReplay, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryReplay,
};

#[test]
fn semantic_tree_sitter_query_row_replay_renders_same_compact_surface() {
    let output = render_semantic_tree_sitter_query_rows_stdout(&ClientDbSyntaxQueryReplay {
        generation_id: CacheGenerationId::from("syntax-row"),
        language_id: LanguageId::from("rust"),
        grammar_id: "tree-sitter-rust".to_string(),
        grammar_profile_version: "2026-06-04.v1".to_string(),
        input_form: "s-expression".to_string(),
        input_kind: ClientDbSyntaxQueryInputKind::Inline,
        compiled_source: "(function_item name: (identifier) @function.name)".to_string(),
        query_ast_fingerprint: "syntax-query-ast-abi:test".to_string(),
        captures: vec!["function.name".to_string()],
        artifact_id: Some(CacheArtifactId::from(
            "semantic-tree-sitter-query/syntax-row.json",
        )),
        packet_bytes: Some(123),
        file_hashes: Vec::new(),
        rows: vec![
            ClientDbSyntaxCaptureReplay {
                match_locator: "src/lib.rs:10:12".to_string(),
                capture_locator: "src/lib.rs:10".to_string(),
                capture_name: "function.name".to_string(),
                capture_node_type: "identifier".to_string().into(),
                item_node_type: "function_item".to_string().into(),
                field: Some("name".to_string()),
                text: "parse_query".to_string(),
            },
            ClientDbSyntaxCaptureReplay {
                match_locator: "src/main.rs:20".to_string(),
                capture_locator: "src/main.rs:20".to_string(),
                capture_name: "function.name".to_string(),
                capture_node_type: "identifier".to_string().into(),
                item_node_type: "function_item".to_string().into(),
                field: Some("name".to_string()),
                text: "main".to_string(),
            },
        ],
    });

    assert_eq!(output, expected_frontier_stdout("rust"));
}

#[test]
fn semantic_tree_sitter_query_row_replay_renders_compact_miss_note() {
    let output = render_semantic_tree_sitter_query_rows_stdout(&ClientDbSyntaxQueryReplay {
        generation_id: CacheGenerationId::from("syntax-row"),
        language_id: LanguageId::from("rust"),
        grammar_id: "tree-sitter-rust".to_string(),
        grammar_profile_version: "2026-06-04.v1".to_string(),
        input_form: "s-expression".to_string(),
        input_kind: ClientDbSyntaxQueryInputKind::Inline,
        compiled_source: "(function_item name: (identifier) @function.name)".to_string(),
        query_ast_fingerprint: "syntax-query-ast-abi:test".to_string(),
        captures: vec!["function.name".to_string()],
        artifact_id: None,
        packet_bytes: None,
        file_hashes: Vec::new(),
        rows: Vec::new(),
    });

    assert_eq!(output, expected_miss_stdout());
}
