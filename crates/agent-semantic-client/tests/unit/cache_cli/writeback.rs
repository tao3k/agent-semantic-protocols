use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientCacheFileHash, LanguageId, ProviderId, ResolvedProvider,
};
use serde_json::{Value, json};

use super::{syntax_query_generation_identity, syntax_query_packet_source};

#[test]
fn syntax_query_generation_identity_uses_ast_abi_not_packet_bytes_or_argv() {
    let provider = rust_provider();
    let export_method = CacheExportMethod::from("query/tree-sitter");
    let file_hashes = vec![ClientCacheFileHash {
        path: "src/lib.rs".to_string(),
        sha256: "abc123".to_string(),
    }];
    let compact_packet = syntax_packet("(function_item name: (identifier) @function.name)", 1);
    let pretty_packet = syntax_packet("(function_item\n  name: (identifier) @function.name)", 99);

    assert_ne!(compact_packet, pretty_packet);
    assert_eq!(
        syntax_query_packet_source(&compact_packet),
        Some("(function_item name: (identifier) @function.name)")
    );

    let compact_identity = syntax_query_generation_identity(
        Path::new("."),
        &provider,
        &export_method,
        &compact_packet,
        Some(&file_hashes),
    )
    .expect("compact identity");
    let pretty_identity = syntax_query_generation_identity(
        Path::new("."),
        &provider,
        &export_method,
        &pretty_packet,
        Some(&file_hashes),
    )
    .expect("pretty identity");

    assert_eq!(compact_identity, pretty_identity);
}

#[test]
fn syntax_query_generation_identity_changes_when_source_hash_changes() {
    let provider = rust_provider();
    let export_method = CacheExportMethod::from("query/tree-sitter");
    let packet = syntax_packet("(function_item name: (identifier) @function.name)", 1);
    let first_hashes = vec![ClientCacheFileHash {
        path: "src/lib.rs".to_string(),
        sha256: "abc123".to_string(),
    }];
    let changed_hashes = vec![ClientCacheFileHash {
        path: "src/lib.rs".to_string(),
        sha256: "def456".to_string(),
    }];

    let first_identity = syntax_query_generation_identity(
        Path::new("."),
        &provider,
        &export_method,
        &packet,
        Some(&first_hashes),
    )
    .expect("first identity");
    let changed_identity = syntax_query_generation_identity(
        Path::new("."),
        &provider,
        &export_method,
        &packet,
        Some(&changed_hashes),
    )
    .expect("changed identity");

    assert_ne!(first_identity, changed_identity);
}

fn syntax_packet(input: &str, volatile_id: u64) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": input,
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"],
                "volatileId": volatile_id
            }
        },
        "matches": []
    })
}

fn rust_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
    }
}
