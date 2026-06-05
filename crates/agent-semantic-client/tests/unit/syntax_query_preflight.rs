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
fn ignores_catalog_and_owner_queries() {
    let catalog_request = query_request(vec![
        "--catalog".to_string(),
        "declarations".to_string(),
        ".".to_string(),
    ]);
    let owner_request = query_request(vec![
        "src/lib.rs".to_string(),
        "--query".to_string(),
        "load".to_string(),
        ".".to_string(),
    ]);

    validate_syntax_query_request(&catalog_request).expect("catalog query");
    validate_syntax_query_request(&owner_request).expect("owner query");
}

fn query_request(forwarded_args: Vec<String>) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, ".")
        .with_language("rust")
        .with_forwarded_args(forwarded_args)
}
