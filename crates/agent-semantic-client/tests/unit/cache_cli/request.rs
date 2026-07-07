use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, LanguageId, ProviderExecution, ProviderId,
    ResolvedProvider,
};

use super::{
    exact_request_fingerprint, prompt_output_render_abi_provenance, request_export_method,
    request_lookup_fingerprint, syntax_query_cache_provenance,
};

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
        source_roots: vec!["src".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        ignored_path_prefixes: Vec::new(),
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
fn search_prime_request_fingerprint_records_prompt_output_render_abi() {
    let prime = prompt_output_render_abi_provenance(&CacheExportMethod::from("search/prime"));
    let package = prompt_output_render_abi_provenance(&CacheExportMethod::from("search/package"));
    let query_code = prompt_output_render_abi_provenance(&CacheExportMethod::from("query/code"));

    assert!(prime.starts_with("prompt-output-render-abi:fnv64:"));
    assert_eq!(prime, package);
    assert_eq!(query_code, "prompt-output-render-abi:none");
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
fn selector_code_query_uses_code_export_method_not_direct_source_read() {
    let request = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--selector".to_string(),
        "src/lib.rs:10:20".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ]);

    assert_eq!(
        request_export_method(&request)
            .expect("export method")
            .as_str(),
        "query/code"
    );
}

#[test]
fn split_from_hook_direct_source_read_keeps_audit_method() {
    let request = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.rs:10:20".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ]);

    assert_eq!(
        request_export_method(&request)
            .expect("export method")
            .as_str(),
        "query/direct-source-read"
    );
    assert!(request.is_hook_direct_source_read());
}

#[test]
fn inline_from_hook_direct_source_read_keeps_audit_method() {
    let request = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--from-hook=direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.rs:10:20".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ]);

    assert_eq!(
        request_export_method(&request)
            .expect("export method")
            .as_str(),
        "query/direct-source-read"
    );
    assert!(request.is_hook_direct_source_read());
    assert!(request.is_source_content_output());
}

#[test]
fn selector_code_query_is_source_content_output() {
    let split_selector = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--selector".to_string(),
        "src/lib.rs:1:12".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);
    let inline_selector = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--selector=src/lib.rs:1:12".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);
    let json_selector = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--selector".to_string(),
        "src/lib.rs:1:12".to_string(),
        "--code".to_string(),
        "--json".to_string(),
        ".".to_string(),
    ]);
    let tree_sitter_code = ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:12".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);

    assert!(split_selector.is_source_content_output());
    assert!(inline_selector.is_source_content_output());
    assert!(tree_sitter_code.is_source_content_output());
    assert!(!json_selector.is_source_content_output());
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

#[test]
fn search_request_fingerprint_ignores_trailing_workspace_markers() {
    let provider = rust_provider();
    let project_root = std::env::current_dir().expect("current dir");
    let export_method = CacheExportMethod::from("search/prime");
    let base_args = vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ];
    let dot_args = vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ];
    let absolute_workspace_args = vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        project_root.display().to_string(),
    ];

    let base_fingerprint =
        exact_request_fingerprint(&provider, &project_root, &export_method, &base_args);
    assert_eq!(
        base_fingerprint,
        exact_request_fingerprint(&provider, &project_root, &export_method, &dot_args)
    );
    assert_eq!(
        base_fingerprint,
        exact_request_fingerprint(
            &provider,
            &project_root,
            &export_method,
            &absolute_workspace_args
        )
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
        source_roots: vec!["src".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        ignored_path_prefixes: Vec::new(),
    }
}
