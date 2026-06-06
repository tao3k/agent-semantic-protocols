use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, LanguageId, ProviderExecution, ProviderId,
    ResolvedProvider,
};

use super::{exact_request_fingerprint, request_lookup_fingerprint, syntax_query_cache_provenance};

#[test]
fn syntax_query_cache_provenance_records_compiled_abi_plan() {
    let provider = rust_provider();
    let provenance = syntax_query_cache_provenance(
        &provider,
        &[
            "--treesitter-query".to_string(),
            "(function_item name: (identifier) @function.name)".to_string(),
            ".".to_string(),
        ],
    )
    .expect("syntax provenance");

    assert!(provenance.starts_with("syntax-query-ast-abi:"));
}

#[test]
fn syntax_query_cache_provenance_changes_when_predicate_abi_changes() {
    let provider = rust_provider();
    let base = syntax_query_cache_provenance(
        &provider,
        &[
            "--treesitter-query".to_string(),
            "(function_item name: (identifier) @function.name)".to_string(),
            ".".to_string(),
        ],
    )
    .expect("base syntax provenance");
    let with_predicate = syntax_query_cache_provenance(&provider, &[
        "--treesitter-query".to_string(),
        r#"(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))"#
            .to_string(),
        ".".to_string(),
    ])
    .expect("predicate syntax provenance");

    assert_ne!(base, with_predicate);
}

#[test]
fn syntax_query_cache_provenance_records_builtin_catalog_abi_plan() {
    let provider = rust_provider();
    let provenance = syntax_query_cache_provenance(
        &provider,
        &[
            "--catalog".to_string(),
            "declarations".to_string(),
            ".".to_string(),
        ],
    )
    .expect("catalog syntax provenance");

    assert!(provenance.starts_with("syntax-query-ast-abi:"));
}

#[test]
fn tree_sitter_request_fingerprint_changes_when_compiled_abi_plan_changes() {
    let provider = ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
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

#[test]
fn tree_sitter_generation_probe_defers_to_ast_row_lookup() {
    let provider = rust_provider();
    let export_method = CacheExportMethod::from("query/tree-sitter");
    let request = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        ".".to_string(),
    ]);

    assert_eq!(
        request_lookup_fingerprint(&provider, Path::new("."), &export_method, &request),
        None
    );
}

#[test]
fn non_tree_sitter_generation_probe_uses_exact_request_fingerprint() {
    let provider = rust_provider();
    let export_method = CacheExportMethod::from("search/prime");
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);
    let different_request =
        ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
            "owner".to_string(),
            "src/lib.rs".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);

    let fingerprint =
        request_lookup_fingerprint(&provider, Path::new("."), &export_method, &request)
            .expect("request fingerprint");
    assert_eq!(
        fingerprint,
        exact_request_fingerprint(
            &provider,
            Path::new("."),
            &export_method,
            &request.forwarded_args,
        )
    );
    assert_ne!(
        fingerprint,
        request_lookup_fingerprint(
            &provider,
            Path::new("."),
            &export_method,
            &different_request,
        )
        .expect("different request fingerprint")
    );
}

fn rust_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
    }
}
