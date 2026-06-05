use crate::cache_replay::{
    query_packet_matches_request, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout, semantic_tree_sitter_query_packet_matches_request,
};
use agent_semantic_client_core::{
    CacheArtifactId, CacheGenerationId, ClientMethod, ClientRequest, LanguageId,
};
use agent_semantic_client_db::{
    ClientDbSyntaxCaptureReplay, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryReplay,
};
use serde_json::{Value, json};

#[test]
fn query_packet_replay_requires_matching_owner_and_query() {
    let request = query_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );
    let term_request = term_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );
    let code_request = code_query_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );

    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &request,
        )
        .is_some()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &term_request,
        )
        .is_some()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/cache_cli/writeback.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &request,
        )
        .is_none()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "write_prompt_output_cache_after_provider_success"
            ),
            &request,
        )
        .is_none()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &code_request,
        )
        .is_none()
    );
}

fn query_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--query", query)
}

fn term_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--term", query)
}

fn code_query_request(owner_path: &str, query: &str) -> ClientRequest {
    let mut request = request_with_query_flag(owner_path, "--query", query);
    request.forwarded_args.insert(3, "--code".to_string());
    request
}

fn request_with_query_flag(owner_path: &str, flag: &str, query: &str) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        owner_path.to_string(),
        flag.to_string(),
        query.to_string(),
        ".".to_string(),
    ])
}

fn query_packet(owner_path: &str, query: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "method": "query/owner-items",
        "ownerPath": owner_path,
        "query": query,
        "matches": []
    })
}

#[test]
fn semantic_tree_sitter_query_replay_requires_exact_query_selector_and_no_code() {
    let source = "(function_item name: (identifier) @function.name)";
    let selector = "src/lib.rs:1:80";
    let request = syntax_request(source, selector, false);

    assert!(
        semantic_tree_sitter_query_packet_matches_request(
            &syntax_packet(source, selector, false),
            &request,
        )
        .is_some()
    );
    assert!(
        semantic_tree_sitter_query_packet_matches_request(
            &syntax_packet(
                "(struct_item name: (type_identifier) @type.name)",
                selector,
                false
            ),
            &request,
        )
        .is_none()
    );
    assert!(
        semantic_tree_sitter_query_packet_matches_request(
            &syntax_packet(source, "src/other.rs:1:80", false),
            &request,
        )
        .is_none()
    );
    assert!(
        semantic_tree_sitter_query_packet_matches_request(
            &syntax_packet(source, selector, false),
            &syntax_request(source, selector, true),
        )
        .is_none()
    );
    assert!(
        semantic_tree_sitter_query_packet_matches_request(
            &syntax_packet(source, selector, true),
            &request,
        )
        .is_none()
    );
}

#[test]
fn semantic_tree_sitter_query_replay_renders_frontier_graph_output() {
    let output = render_semantic_tree_sitter_query_stdout(&json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": [
            {
                "id": "m1",
                "range": {"path": "src/lib.rs", "lineRange": "10:12"},
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "fields": {"symbol": "parse_query"}
                    }
                ]
            },
            {
                "id": "m2",
                "captures": [
                    {
                        "id": "c2",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/main.rs", "lineRange": {"start": 20, "end": 20}},
                        "fields": {"name": "main"}
                    }
                ]
            }
        ]
    }))
    .expect("syntax replay stdout");

    assert_eq!(
        output,
        "[query-treesitter] root=. lang=unknown pattern=function_item/name capture=function.name alg=syntax-capture-frontier\n\
legend: aliases ID:kind; node ID=kind:role(value)!next; ts=node/field; frontier ID.next\n\
aliases=G:query,Q:tsquery,C:capture,I:item,O:owner\n\n\
Q=tsquery:pattern(function_item/name)!query\n\
C=capture:function.name(parse_query)@src/lib.rs:10!code ts=identifier/name\n\
I=item:fn(parse_query)@src/lib.rs:10:12!code ts=function_item\n\
C2=capture:function.name(main)@src/main.rs:20!code ts=identifier/name\n\
I2=item:fn(main)@src/main.rs:20!code ts=function_item\n\n\
G>{Q:selects}\n\
Q>{C:captures,C2:captures}\n\
C>{I:enclosing-item}\n\
C2>{I2:enclosing-item}\n\n\
omit=code,full-node-list,capture-text\n\
rank=I,I2\n\
frontier=I.code,I2.code\n\
avoid=broad-code-output,raw-read\n"
    );
}

#[test]
fn semantic_tree_sitter_query_replay_renders_compact_miss_note() {
    let output = render_semantic_tree_sitter_query_stdout(&json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": []
    }))
    .expect("syntax miss stdout");

    assert_eq!(
        output,
        "|syntax-query inputForm=s-expression input=inline grammar=tree-sitter-rust grammarProfile=2026-06-04.v1 dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount=1 captures=function.name\n"
    );
}

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

    assert_eq!(
        output,
        "[query-treesitter] root=. lang=rust pattern=function_item/name capture=function.name alg=syntax-capture-frontier\n\
legend: aliases ID:kind; node ID=kind:role(value)!next; ts=node/field; frontier ID.next\n\
aliases=G:query,Q:tsquery,C:capture,I:item,O:owner\n\n\
Q=tsquery:pattern(function_item/name)!query\n\
C=capture:function.name(parse_query)@src/lib.rs:10!code ts=identifier/name\n\
I=item:fn(parse_query)@src/lib.rs:10:12!code ts=function_item\n\
C2=capture:function.name(main)@src/main.rs:20!code ts=identifier/name\n\
I2=item:fn(main)@src/main.rs:20!code ts=function_item\n\n\
G>{Q:selects}\n\
Q>{C:captures,C2:captures}\n\
C>{I:enclosing-item}\n\
C2>{I2:enclosing-item}\n\n\
omit=code,full-node-list,capture-text\n\
rank=I,I2\n\
frontier=I.code,I2.code\n\
avoid=broad-code-output,raw-read\n"
    );
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

    assert_eq!(
        output,
        "|syntax-query inputForm=s-expression input=inline grammar=tree-sitter-rust grammarProfile=2026-06-04.v1 dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount=1 captures=function.name\n"
    );
}

fn syntax_request(source: &str, selector: &str, code: bool) -> ClientRequest {
    let mut args = vec![
        "--treesitter-query".to_string(),
        source.to_string(),
        "--selector".to_string(),
        selector.to_string(),
        ".".to_string(),
    ];
    if code {
        args.push("--code".to_string());
    }
    ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(args)
}

fn syntax_packet(source: &str, selector: &str, code_output: bool) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "query": {
            "input": source,
            "inputForm": "s-expression",
            "fields": {
                "selector": selector,
                "codeOutput": code_output
            }
        },
        "matches": []
    })
}
