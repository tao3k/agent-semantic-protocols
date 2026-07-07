use serde_json::{Value, json};

use crate::rust_harness_activation::support::temp_project_root;

use super::support::run_hook_decision;

#[test]
fn cli_hook_replay_blocks_encoded_payload_escape_surfaces() {
    struct Case {
        name: &'static str,
        payload: Value,
        reason_kind: &'static str,
    }

    let read_arguments = json!({"path": "src/lib.rs"}).to_string();
    let command_arguments = json!({"cmd": "cat src/lib.rs"}).to_string();
    let direct_source_route = json!([
        "asp",
        "rust",
        "search",
        "owner",
        "src/lib.rs",
        "items",
        "--workspace",
        ".",
        "--view",
        "seeds"
    ]);

    let cases = vec![
        Case {
            name: "hook-escape-lower-bash",
            payload: json!({
                "toolName": "bash",
                "toolInput": {"cmd": "cat src/lib.rs"}
            }),
            reason_kind: "bulk-source-dump",
        },
        Case {
            name: "hook-escape-lower-shell",
            payload: json!({
                "toolName": "shell",
                "toolInput": {"cmd": "cat src/lib.rs"}
            }),
            reason_kind: "bulk-source-dump",
        },
        Case {
            name: "hook-escape-top-level-tool-uses",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "tool_uses": [{
                    "recipient_name": "Read",
                    "parameters": {"path": "src/lib.rs"}
                }]
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-top-level-tools",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "tools": [{
                    "recipient_name": "Read",
                    "parameters": {"path": "src/lib.rs"}
                }]
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-function-arguments-json",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [{
                        "function": {
                            "name": "Read",
                            "arguments": read_arguments
                        }
                    }]
                }
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-tool-calls-function-json",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_calls": [{
                        "function": {
                            "name": "Read",
                            "arguments": read_arguments
                        }
                    }]
                }
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-top-level-function-json",
            payload: json!({
                "toolName": "Read",
                "toolInput": {
                    "function": {
                        "name": "Read",
                        "arguments": read_arguments
                    }
                }
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-arguments-json-string",
            payload: json!({
                "toolName": "Read",
                "arguments": read_arguments
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-parameters-json-string",
            payload: json!({
                "toolName": "Read",
                "parameters": read_arguments
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-command-arguments-json-string",
            payload: json!({
                "toolName": "functions.exec_command",
                "arguments": command_arguments
            }),
            reason_kind: "bulk-source-dump",
        },
        Case {
            name: "hook-escape-shell-argv",
            payload: json!({
                "toolName": "functions.exec_command",
                "toolInput": {"args": ["sh", "-c", "cat src/lib.rs"]}
            }),
            reason_kind: "bulk-source-dump",
        },
        Case {
            name: "hook-escape-files-object-array",
            payload: json!({
                "toolName": "Read",
                "toolInput": {"files": [{"path": "src/lib.rs"}]}
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-path-object",
            payload: json!({
                "toolName": "Read",
                "toolInput": {"path": {"path": "src/lib.rs"}}
            }),
            reason_kind: "direct-source-read",
        },
        Case {
            name: "hook-escape-parallel-input-json",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [{
                        "recipient_name": "Read",
                        "input": read_arguments
                    }]
                }
            }),
            reason_kind: "direct-source-read",
        },
    ];

    for case in cases {
        let root = temp_project_root(case.name);
        let decision = run_hook_decision(&root, "pre-tool", case.payload);
        assert_eq!(decision["decision"], "deny", "case={}", case.name);
        assert_eq!(
            decision["reasonKind"], case.reason_kind,
            "case={}",
            case.name
        );
        assert_eq!(
            decision["routes"][0]["argv"], direct_source_route,
            "case={}",
            case.name
        );
        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }
}

#[test]
fn cli_hook_replay_allows_safe_encoded_command() {
    let root = temp_project_root("hook-safe-encoded-command");
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "toolName": "functions.exec_command",
            "arguments": json!({"cmd": "echo ok"}).to_string()
        }),
    );

    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
