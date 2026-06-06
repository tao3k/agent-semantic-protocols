use crate::cache_replay::{
    render_semantic_tree_sitter_query_stdout, semantic_tree_sitter_query_packet_matches_request,
};
use agent_semantic_client_core::{ClientMethod, ClientRequest};
use serde_json::{Value, json};

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

    assert_eq!(output, expected_frontier_stdout("unknown"));
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

    assert_eq!(output, expected_miss_stdout());
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

pub(super) fn expected_frontier_stdout(language: &str) -> String {
    format!(
        "[query-treesitter] root=. lang={language} pattern=function_item/name capture=function.name alg=syntax-capture-frontier\n\
legend: aliases ID:kind; node ID=kind:role(value)!next; ts=node/field; frontier ID.next\n\
aliases=G:query,Q:tsquery,C:capture,I:item,O:owner\n\n\
Q=tsquery:pattern(function_item/name)!query\n\
C=capture:function.name(parse_query)@src/lib.rs:10!code ts=identifier/name\n\
I=item:fn(parse_query)@src/lib.rs:10:12!code ts=function_item\n\
C2=capture:function.name(main)@src/main.rs:20!code ts=identifier/name\n\
I2=item:fn(main)@src/main.rs:20!code ts=function_item\n\n\
G>{{Q:selects}}\n\
Q>{{C:captures,C2:captures}}\n\
C>{{I:enclosing-item}}\n\
C2>{{I2:enclosing-item}}\n\n\
omit=code,full-node-list,capture-text\n\
rank=I,I2\n\
frontier=I.code,I2.code\n\
avoid=broad-code-output,raw-read\n"
    )
}

pub(super) fn expected_miss_stdout() -> &'static str {
    "|syntax-query inputForm=s-expression input=inline grammar=tree-sitter-rust grammarProfile=2026-06-04.v1 dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount=1 captures=function.name\n"
}
