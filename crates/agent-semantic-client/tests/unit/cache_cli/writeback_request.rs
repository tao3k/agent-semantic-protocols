// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::PathBuf;

use agent_semantic_client_core::{ClientMethod, ClientRequest, LanguageId};

use super::{
    request_prompt_output_writeback_method, request_search_packet_writeback_method,
    request_syntax_query_writeback_method,
};

#[test]
fn query_selector_source_projection_is_not_prompt_output_writeback() {
    let request = ClientRequest::new(ClientMethod::Query, PathBuf::from("."))
        .with_language(LanguageId::from("gerbil-scheme"))
        .with_forwarded_args(vec![
            "--selector".to_string(),
            "src/support/io.ss:120-140".to_string(),
            "--projection".to_string(),
            "source".to_string(),
        ]);

    assert!(request_prompt_output_writeback_method(&request).is_none());
    assert!(request_search_packet_writeback_method(&request).is_none());
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
    assert!(request_search_packet_writeback_method(&request).is_none());
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
