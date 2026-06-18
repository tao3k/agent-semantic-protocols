use std::path::PathBuf;

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};

use super::{
    request_prompt_output_writeback_method, request_query_packet_writeback_method,
    request_syntax_query_writeback_method,
};

#[test]
fn query_selector_content_is_prompt_output_writeback() {
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("."))
        .with_language(LanguageId::from("gerbil-scheme"))
        .with_forwarded_args(vec![
            "--selector".to_string(),
            "src/support/io.ss:120-140".to_string(),
            "--content".to_string(),
        ]);

    let export_method =
        request_prompt_output_writeback_method(&request).expect("prompt output writeback");

    assert_eq!(export_method.as_str(), "query/owner-items");
    assert!(request_query_packet_writeback_method(&request).is_none());
}

#[test]
fn query_selector_json_is_not_prompt_output_writeback() {
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("."))
        .with_language(LanguageId::from("gerbil-scheme"))
        .with_forwarded_args(vec![
            "--selector".to_string(),
            "src/support/io.ss:120-140".to_string(),
            "--json".to_string(),
        ]);

    assert!(request_prompt_output_writeback_method(&request).is_none());
    assert!(request_query_packet_writeback_method(&request).is_none());
}

#[test]
fn query_selector_tree_sitter_is_syntax_writeback() {
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("."))
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "--treesitter-query".to_string(),
            "(function_item)".to_string(),
            "--selector".to_string(),
            "src/lib.rs:1-3".to_string(),
        ]);

    assert!(request_prompt_output_writeback_method(&request).is_none());
    let export_method =
        request_syntax_query_writeback_method(&request).expect("syntax query writeback");

    assert_eq!(export_method.as_str(), "query/tree-sitter");
}
