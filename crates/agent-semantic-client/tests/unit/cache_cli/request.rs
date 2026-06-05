use std::path::Path;

use agent_semantic_client_core::{CacheExportMethod, LanguageId, ProviderId, ResolvedProvider};

use super::{exact_request_fingerprint, syntax_query_cache_provenance};

#[test]
fn syntax_query_cache_provenance_records_compiled_abi_plan() {
    let provenance = syntax_query_cache_provenance(&[
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        ".".to_string(),
    ])
    .expect("syntax provenance");

    assert!(provenance.starts_with("syntax-query-ast-abi:"));
}

#[test]
fn syntax_query_cache_provenance_changes_when_predicate_abi_changes() {
    let base = syntax_query_cache_provenance(&[
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        ".".to_string(),
    ])
    .expect("base syntax provenance");
    let with_predicate = syntax_query_cache_provenance(&[
        "--treesitter-query".to_string(),
        r#"(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))"#
            .to_string(),
        ".".to_string(),
    ])
    .expect("predicate syntax provenance");

    assert_ne!(base, with_predicate);
}

#[test]
fn tree_sitter_request_fingerprint_changes_when_compiled_abi_plan_changes() {
    let provider = ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
    };
    let export_method = CacheExportMethod::from("query/tree-sitter");
    let function_fingerprint = exact_request_fingerprint(
        &provider,
        Path::new("."),
        &export_method,
        &[
            "--treesitter-query".to_string(),
            "(function_item name: (identifier) @function.name)".to_string(),
            ".".to_string(),
        ],
    );
    let struct_fingerprint = exact_request_fingerprint(
        &provider,
        Path::new("."),
        &export_method,
        &[
            "--treesitter-query".to_string(),
            "(struct_item name: (type_identifier) @type.name)".to_string(),
            ".".to_string(),
        ],
    );

    assert_ne!(function_fingerprint, struct_fingerprint);
}
