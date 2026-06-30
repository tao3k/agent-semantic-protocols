use serde_json::json;

use crate::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout,
};

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

#[test]
fn semantic_tree_sitter_query_row_replay_renders_same_compact_surface() {
    let output = render_semantic_tree_sitter_query_rows_stdout(SyntaxQueryRowsReplay {
        language_id: "rust",
        grammar_id: "tree-sitter-rust",
        grammar_profile_version: "2026-06-04.v1",
        input_form: "s-expression",
        input_kind: "inline",
        compiled_source: "(function_item name: (identifier) @function.name)",
        captures: &["function.name".to_string()],
        rows: vec![
            SyntaxQueryReplayCapture {
                match_locator: "src/lib.rs:10:12".to_string(),
                capture_locator: "src/lib.rs:10".to_string(),
                capture_name: "function.name".to_string(),
                capture_node_type: Some("identifier".to_string()),
                item_node_type: Some("function_item".to_string()),
                field: Some("name".to_string()),
                text: "parse_query".to_string(),
            },
            SyntaxQueryReplayCapture {
                match_locator: "src/main.rs:20".to_string(),
                capture_locator: "src/main.rs:20".to_string(),
                capture_name: "function.name".to_string(),
                capture_node_type: Some("identifier".to_string()),
                item_node_type: Some("function_item".to_string()),
                field: Some("name".to_string()),
                text: "main".to_string(),
            },
        ],
    });

    assert_eq!(output, expected_frontier_stdout("rust"));
}

fn expected_frontier_stdout(language: &str) -> String {
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

fn expected_miss_stdout() -> &'static str {
    "|syntax-query inputForm=s-expression input=inline grammar=tree-sitter-rust grammarProfile=2026-06-04.v1 dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount=1 captures=function.name\n"
}
