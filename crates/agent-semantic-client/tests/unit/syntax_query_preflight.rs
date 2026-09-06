// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_core::ClientMethod;
use agent_semantic_client_core::ClientRequest;

use crate::syntax_query_preflight::validate_syntax_query_request;

#[test]
fn accepts_valid_inline_tree_sitter_query() {
    let request = query_request(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        ".".to_string(),
    ]);

    validate_syntax_query_request(&request).expect("valid query");
}

#[test]
fn rejects_exact_projection_directory_selector_before_provider_execution() {
    let request = query_request(vec![
        "--selector".to_string(),
        "src".to_string(),
        "--query".to_string(),
        "owner items".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--projection".to_string(),
        "source".to_string(),
    ]);

    let error =
        validate_syntax_query_request(&request).expect_err("directory selector should fail");

    assert!(
        error.contains("exact query requires a parser-owned structural selector"),
        "{error}"
    );
    assert!(error.contains("`src` is a directory"), "{error}");
}

#[test]
fn rejects_exact_projection_file_selector_before_provider_execution() {
    let request = query_request(vec![
        "--selector".to_string(),
        "src/lib.rs".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--projection".to_string(),
        "source".to_string(),
    ]);

    let error = validate_syntax_query_request(&request).expect_err("file selector should fail");

    assert!(
        error.contains("invalid exact-query selector `src/lib.rs`"),
        "{error}"
    );
    assert!(error.contains("selectorState=file-selector"), "{error}");
    assert!(error.contains("allowed=false"), "{error}");
    assert!(
        error.contains("requiredSelector=rust://src/lib.rs#item/<kind>/<name>"),
        "{error}"
    );
    assert!(!error.contains("next"), "{error}");
    assert!(!error.contains("recommend"), "{error}");
}

#[test]
fn rejects_non_structural_selector_without_manifest_extension_defaults() {
    let request = query_request(vec![
        "--selector".to_string(),
        "src/lib.future-language".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--projection".to_string(),
        "source".to_string(),
    ]);

    let error =
        validate_syntax_query_request(&request).expect_err("non-structural selector should fail");

    assert!(
        error.contains("invalid exact-query selector `src/lib.future-language`"),
        "{error}"
    );
    assert!(error.contains("requiredSelector=rust://"), "{error}");
}

#[test]
fn rejects_stale_exact_selector_path_before_provider_execution() {
    let request = query_request(vec![
        "--selector".to_string(),
        "rust://crates/agent-semantic-client/src/search_pipe_source.rs#item/function/collect_search_pipe_auto_acquisition".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--projection".to_string(),
        "source".to_string(),
    ]);

    let error =
        validate_syntax_query_request(&request).expect_err("stale exact selector should fail");

    assert!(
        error.contains("stale-index")
            || error.contains("selector path does not exist under --workspace"),
        "{error}"
    );
    assert!(
        error.contains("crates/agent-semantic-client/src/search_pipe_source.rs"),
        "{error}"
    );
}

#[test]
fn rejects_missing_query_owner_path_under_workspace_before_provider_execution() {
    let request = query_request(vec!["src/types/facade.ss".to_string()]);

    let error =
        validate_syntax_query_request(&request).expect_err("missing owner path should fail");

    assert!(
        error.contains("query owner path does not exist under --workspace: src/types/facade.ss"),
        "{error}"
    );
}

#[test]
fn accepts_existing_query_owner_path_under_workspace() {
    let request = query_request(vec!["src/lib.rs".to_string()]);

    validate_syntax_query_request(&request).expect("existing owner path");
}

#[test]
fn accepts_json_positional_project_root_without_owner_path_preflight() {
    let request = query_request(vec![
        "--catalog".to_string(),
        "calls".to_string(),
        "--selector".to_string(),
        "src/corpus.rs".to_string(),
        "--json".to_string(),
        "/tmp/tree-sitter-corpus-project".to_string(),
    ]);

    validate_syntax_query_request(&request).expect("json positional project root");
}

#[test]
fn rejects_invalid_inline_tree_sitter_query_before_provider_execution() {
    let request = query_request(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name".to_string(),
        ".".to_string(),
    ]);

    let error = validate_syntax_query_request(&request).expect_err("invalid query");

    assert_eq!(
        error,
        "invalid tree-sitter query ABI source before provider execution: unclosed query pattern"
    );
}

#[test]
fn accepts_builtin_catalog_query_before_provider_execution() {
    let catalog_request = query_request(vec![
        "--catalog".to_string(),
        "declarations".to_string(),
        ".".to_string(),
    ]);

    validate_syntax_query_request(&catalog_request).expect("catalog query");
}

#[test]
fn accepts_native_flow_lite_catalog_without_tree_sitter_preflight() {
    let catalog_request = query_request(vec![
        "--catalog".to_string(),
        "flow-lite".to_string(),
        "--where".to_string(),
        "source.call=payload_string sink.constructs=ToolAction scope.fn=collect_tool_actions"
            .to_string(),
        ".".to_string(),
    ]);

    validate_syntax_query_request(&catalog_request).expect("native flow-lite catalog");
}

#[test]
fn rejects_unknown_builtin_catalog_query_before_provider_execution() {
    let catalog_request = query_request(vec![
        "--catalog".to_string(),
        "missing".to_string(),
        ".".to_string(),
    ]);

    let error = validate_syntax_query_request(&catalog_request).expect_err("unknown catalog");

    assert_eq!(
        error,
        "unknown built-in tree-sitter query catalog `missing` for language `rust`"
    );
}

#[test]
fn ignores_owner_queries() {
    let owner_request = query_request(vec![
        "src/lib.rs".to_string(),
        "--query".to_string(),
        "load".to_string(),
        ".".to_string(),
    ]);

    validate_syntax_query_request(&owner_request).expect("owner query");
}

fn query_request(forwarded_args: Vec<String>) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, env!("CARGO_MANIFEST_DIR"))
        .with_language("rust")
        .with_forwarded_args(forwarded_args)
}
