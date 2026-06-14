use agent_semantic_client_core::{ClientMethod, ClientRequest};

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
fn rejects_tree_sitter_query_code_output_without_exact_selector() {
    let request = query_request(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        "--code".to_string(),
    ]);

    let error = validate_syntax_query_request(&request).expect_err("missing exact selector");

    assert_eq!(
        error,
        "tree-sitter query --code requires an exact --selector; run without --code for a capture frontier or add --selector <path-or-range> for pure code"
    );
}

#[test]
fn accepts_tree_sitter_query_code_output_with_exact_selector() {
    let request = query_request(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:10".to_string(),
        "--code".to_string(),
    ]);

    validate_syntax_query_request(&request).expect("exact selector tree-sitter code query");
}

#[test]
fn rejects_query_code_trailing_project_root_before_cache_replay() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:2".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);

    let error = validate_syntax_query_request(&request).expect_err("trailing project root");

    assert_eq!(
        error,
        "query/search --code does not accept a trailing PROJECT_ROOT; use --workspace PROJECT_ROOT"
    );
}

#[test]
fn rejects_search_code_trailing_project_root_before_cache_replay() {
    let request = ClientRequest::new(ClientMethod::Search, ".")
        .with_language("rust")
        .with_forwarded_args(vec![
            "owner".to_string(),
            "src/lib.rs".to_string(),
            "items".to_string(),
            "--query".to_string(),
            "target".to_string(),
            "--code".to_string(),
            ".".to_string(),
        ]);

    let error = validate_syntax_query_request(&request).expect_err("trailing project root");

    assert_eq!(
        error,
        "query/search --code does not accept a trailing PROJECT_ROOT; use --workspace PROJECT_ROOT"
    );
}

#[test]
fn accepts_query_code_with_workspace_before_cache_replay() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:2".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ]);

    validate_syntax_query_request(&request).expect("workspace code query");
}

#[test]
fn rejects_missing_query_owner_path_under_workspace_before_provider_execution() {
    let request = query_request(vec![
        "src/types/facade.ss".to_string(),
        "--names-only".to_string(),
    ]);

    let error =
        validate_syntax_query_request(&request).expect_err("missing owner path should fail");

    assert!(
        error.contains("query owner path does not exist under --workspace: src/types/facade.ss"),
        "{error}"
    );
}

#[test]
fn accepts_existing_query_owner_path_under_workspace() {
    let request = query_request(vec!["src/lib.rs".to_string(), "--names-only".to_string()]);

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
    ClientRequest::new(ClientMethod::Query, ".")
        .with_language("rust")
        .with_forwarded_args(forwarded_args)
}
