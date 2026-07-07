use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::registry;

#[test]
fn codex_shell_source_read_wrappers_are_denied() {
    let source = rust_runner_source_path();
    let extension_source = rust_extension_source_path();
    let python_source = nested_python_source_path();
    let cases = vec![
        (
            "nl-sed-pipeline",
            format!("nl -ba {source} | sed -n '1,40p'"),
            source.clone(),
        ),
        (
            "pipeline-late-cat",
            format!("true | cat {source}"),
            source.clone(),
        ),
        (
            "command-substitution-cat",
            format!("echo $(cat {source})"),
            source.clone(),
        ),
        (
            "process-substitution-sed",
            format!("cat <(sed -n '1,3p' {source})"),
            source.clone(),
        ),
        (
            "conditional-and-sed",
            format!("test -f {source} && sed -n '1,20p' {source}"),
            source.clone(),
        ),
        (
            "conditional-or-python",
            format!(
                "false || python -c \"from pathlib import Path; print(Path('{source}').read_text())\""
            ),
            source.clone(),
        ),
        (
            "awk-range",
            format!("awk 'NR >= 1 && NR <= 40 {{ print }}' {source}"),
            source.clone(),
        ),
        (
            "perl-filter",
            format!("perl -ne 'print if $. <= 40' {source}"),
            source.clone(),
        ),
        (
            "python-read-text",
            format!("python -c \"from pathlib import Path; print(Path('{source}').read_text())\""),
            source.clone(),
        ),
        (
            "node-read-file-sync",
            format!("node -e \"console.log(require('fs').readFileSync('{source}','utf8'))\""),
            source.clone(),
        ),
        (
            "ruby-file-read",
            format!("ruby -e \"puts File.read('{source}')\""),
            source.clone(),
        ),
        (
            "language-extension-source",
            format!("true | cat {extension_source}"),
            extension_source,
        ),
        (
            "nested-python-package-nl-sed-pipeline",
            format!("nl -ba {python_source} | sed -n '1,130p'"),
            python_source,
        ),
    ];
    for (name, command, expected_path) in cases {
        assert_shell_source_dump_denied(name, &command, &expected_path);
    }
}

#[test]
fn codex_shell_git_diff_patch_review_is_allowed() {
    let source = rust_runner_source_path();
    let command = format!("git diff -- {source}");
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            },
            "tool_input": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}

#[test]
fn nl_sed_pipeline_over_python_source_routes_to_python_by_extension_match() {
    let source = nested_python_source_path();
    let command = format!("nl -ba {source} | sed -n '1,130p'");
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            },
            "tool_input": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.language_ids, ["python".to_string()]);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.subject.command.as_deref(), Some(command.as_str()));
    assert!(
        decision.subject.paths.iter().any(|path| path == &source),
        "{:?}",
        decision.subject.paths
    );
}

#[test]
fn interpreter_source_path_without_read_api_is_allowed() {
    let source = rust_runner_source_path();
    let command = format!("python scripts/check.py {source}");
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            },
            "tool_input": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}

#[test]
fn deeply_wrapped_codex_action_source_dump_is_denied() {
    let source = rust_runner_source_path();
    let command = format!("echo $(cat {source})");
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "parameters": {
                    "toolUse": {
                        "input": {
                            "item": {
                                "toolAction": {
                                    "type": "unknown",
                                    "cmd": command,
                                }
                            }
                        }
                    }
                }
            },
            "tool_input": {
                "parameters": {
                    "toolUse": {
                        "input": {
                            "item": {
                                "toolAction": {
                                    "type": "unknown",
                                    "cmd": command,
                                }
                            }
                        }
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert!(
        decision.subject.paths.iter().any(|path| path == &source),
        "{:?}",
        decision.subject.paths
    );
}

fn rust_runner_source_path() -> String {
    [
        "languages",
        "rust-lang-project-harness",
        "src",
        "cli",
        &format!("runner.{}", "rs"),
    ]
    .join("/")
}

fn rust_extension_source_path() -> String {
    ["scratch", "probe", &format!("extension_only.{}", "rs")].join("/")
}

fn nested_python_source_path() -> String {
    [
        "packages",
        "python",
        "tools",
        "src",
        "tools",
        "semantic_sandtable",
        &format!("step_agent_cli.{}", "py"),
    ]
    .join("/")
}

fn assert_shell_source_dump_denied(name: &str, command: &str, expected_path: &str) {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            },
            "tool_input": {
                "item": {
                    "action": {
                        "type": "unknown",
                        "cmd": command,
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{name}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump, "{name}");
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner, "{name}");
    assert_eq!(decision.subject.command.as_deref(), Some(command), "{name}");
    assert!(
        decision
            .subject
            .paths
            .iter()
            .any(|path| path == expected_path),
        "{name}: {:?}",
        decision.subject.paths
    );
}
