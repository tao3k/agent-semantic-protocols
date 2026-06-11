use serde_json::{Value, json};

use crate::rust_harness_activation::support::temp_project_root;

use super::support::run_hook_decision;

#[test]
fn cli_hook_replay_blocks_common_source_read_escape_surfaces() {
    struct Case {
        name: &'static str,
        payload: Value,
        reason_kind: &'static str,
        tool_name: &'static str,
        command: Option<&'static str>,
    }

    let direct_source_route = |selector: &str| {
        json!([
            "asp",
            "rust",
            "query",
            "--selector",
            selector,
            "--workspace",
            ".",
            "--code"
        ])
    };

    let cases = vec![
        Case {
            name: "hook-escape-functions-cat",
            payload: json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "cmd": "cat src/lib.rs"
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some("cat src/lib.rs"),
        },
        Case {
            name: "hook-escape-functions-head",
            payload: json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "cmd": "head -40 src/lib.rs"
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some("head -40 src/lib.rs"),
        },
        Case {
            name: "hook-escape-bash-command",
            payload: json!({
                "toolName": "Bash",
                "toolInput": {
                    "command": "sed -n '1,40p' src/lib.rs"
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "Bash",
            command: Some("sed -n '1,40p' src/lib.rs"),
        },
        Case {
            name: "hook-escape-shell-cmd",
            payload: json!({
                "toolName": "Shell",
                "toolInput": {
                    "cmd": "cat src/lib.rs"
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "Shell",
            command: Some("cat src/lib.rs"),
        },
        Case {
            name: "hook-escape-read-file-path",
            payload: json!({
                "toolName": "Read",
                "toolInput": {
                    "file_path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-read-files-array",
            payload: json!({
                "toolName": "Read",
                "toolInput": {
                    "files": ["README.md", "src/lib.rs"]
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-read-parameters-path",
            payload: json!({
                "toolName": "Read",
                "parameters": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-read-input-path",
            payload: json!({
                "toolName": "Read",
                "input": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-read-arguments-path",
            payload: json!({
                "toolName": "Read",
                "arguments": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-read-file-tool",
            payload: json!({
                "toolName": "read_file",
                "toolInput": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "read_file",
            command: None,
        },
        Case {
            name: "hook-escape-functions-read-file",
            payload: json!({
                "toolName": "functions.read_file",
                "toolInput": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "functions.read_file",
            command: None,
        },
        Case {
            name: "hook-escape-mcp-read-file",
            payload: json!({
                "toolName": "mcp__filesystem__read_file",
                "toolInput": {
                    "path": "src/lib.rs"
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "mcp__filesystem__read_file",
            command: None,
        },
        Case {
            name: "hook-escape-exec-args-array",
            payload: json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "args": ["cat", "src/lib.rs"]
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some("cat src/lib.rs"),
        },
        Case {
            name: "hook-escape-python-read-text-heredoc",
            payload: json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "cmd": "python3 - <<PY\nfrom pathlib import Path\nprint(Path('src/lib.rs').read_text())\nPY"
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some(
                "python3 - <<PY\nfrom pathlib import Path\nprint(Path('src/lib.rs').read_text())\nPY",
            ),
        },
        Case {
            name: "hook-escape-parallel-cat",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [
                        {
                            "recipient_name": "functions.exec_command",
                            "parameters": {
                                "cmd": "cat src/lib.rs"
                            }
                        }
                    ]
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some("cat src/lib.rs"),
        },
        Case {
            name: "hook-escape-parallel-scans-past-allowed-tool",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [
                        {
                            "recipient_name": "functions.exec_command",
                            "parameters": {
                                "cmd": "pwd"
                            }
                        },
                        {
                            "recipient_name": "functions.exec_command",
                            "parameters": {
                                "cmd": "sed -n '1,40p' src/lib.rs"
                            }
                        }
                    ]
                }
            }),
            reason_kind: "bulk-source-dump",
            tool_name: "functions.exec_command",
            command: Some("sed -n '1,40p' src/lib.rs"),
        },
        Case {
            name: "hook-escape-parallel-read-file",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [
                        {
                            "recipient_name": "Read",
                            "parameters": {
                                "file_path": "src/lib.rs"
                            }
                        }
                    ]
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-parallel-tools-array",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tools": [
                        {
                            "recipient_name": "Read",
                            "parameters": {
                                "path": "src/lib.rs"
                            }
                        }
                    ]
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
        },
        Case {
            name: "hook-escape-parallel-name-input",
            payload: json!({
                "toolName": "multi_tool_use.parallel",
                "toolInput": {
                    "tool_uses": [
                        {
                            "name": "Read",
                            "input": {
                                "path": "src/lib.rs"
                            }
                        }
                    ]
                }
            }),
            reason_kind: "direct-source-read",
            tool_name: "Read",
            command: None,
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
            decision["subject"]["toolName"], case.tool_name,
            "case={}",
            case.name
        );
        if let Some(command) = case.command {
            assert_eq!(
                decision["subject"]["command"], command,
                "case={}",
                case.name
            );
        }
        let expected_selector = match (case.name, case.command) {
            ("hook-escape-functions-head", _) | (_, Some("sed -n '1,40p' src/lib.rs")) => {
                "src/lib.rs:1:40"
            }
            _ => "src/lib.rs",
        };
        assert_eq!(
            decision["routes"][0]["argv"],
            direct_source_route(expected_selector),
            "case={}",
            case.name
        );
        assert!(
            decision["routes"][0]["argv"]
                .as_array()
                .expect("route argv")
                .iter()
                .any(|arg| arg == "--code"),
            "case={}",
            case.name
        );
    }
}

#[test]
fn cli_hook_replay_blocks_absolute_source_path_escape_surface() {
    let root = temp_project_root("hook-escape-read-absolute-path");
    let absolute_path = root.join("src/lib.rs").to_string_lossy().to_string();
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "toolName": "Read",
            "toolInput": {
                "absolute_path": absolute_path
            }
        }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "direct-source-read");
    assert_eq!(decision["subject"]["toolName"], "Read");
    assert_eq!(
        decision["routes"][0]["argv"],
        json!([
            "asp",
            "rust",
            "query",
            "--selector",
            root.join("src/lib.rs").to_string_lossy(),
            "--workspace",
            ".",
            "--code"
        ])
    );

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_blocks_source_glob_escape_surfaces() {
    let cases = vec![
        (
            "hook-escape-rtk-numbered-rust-glob",
            json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "cmd": "rtk read -n *.rs"
                }
            }),
        ),
        (
            "hook-escape-rtk-rust-glob",
            json!({
                "toolName": "functions.exec_command",
                "toolInput": {
                    "cmd": "rtk read *.rs"
                }
            }),
        ),
        (
            "hook-escape-read-rust-glob",
            json!({
                "toolName": "Read",
                "toolInput": {
                    "file_path": "*.rs"
                }
            }),
        ),
        (
            "hook-escape-read-mixed-glob",
            json!({
                "toolName": "Read",
                "toolInput": {
                    "file_path": "*.{rs,py}"
                }
            }),
        ),
    ];

    for (name, payload) in cases {
        let root = temp_project_root(name);
        let decision = run_hook_decision(&root, "pre-tool", payload);
        assert_eq!(decision["decision"], "deny", "case={name}");
        assert_eq!(decision["reasonKind"], "direct-source-read", "case={name}");
        let argv = decision["routes"][0]["argv"]
            .as_array()
            .expect("route argv");
        assert!(
            argv.iter().any(|arg| arg == "query"),
            "case={name} argv={:?}",
            argv
        );
        assert!(
            !argv.iter().any(|arg| arg == "--code"),
            "case={name} argv={:?}",
            argv
        );
    }
}
