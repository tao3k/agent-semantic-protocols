use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::registry;

#[test]
fn claude_grep_tool_routes_as_raw_search() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "pre-tool",
        &json!({
            "toolName": "Grep",
            "tool_name": "Grep",
            "toolInput": {
                "pattern": "HookDecision",
                "glob": "*.rs",
                "path": "crates/agent-semantic-hook/src"
            },
            "tool_input": {
                "pattern": "HookDecision",
                "glob": "*.rs",
                "path": "crates/agent-semantic-hook/src"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert!(
        decision
            .subject
            .command
            .as_deref()
            .unwrap()
            .starts_with("rg --glob")
    );
}

#[test]
fn codex_search_command_action_routes_as_raw_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n -g '*.rs' HookDecision crates/agent-semantic-hook/src",
                    "query": "HookDecision",
                    "path": "crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n -g '*.rs' HookDecision crates/agent-semantic-hook/src",
                    "query": "HookDecision",
                    "path": "crates/agent-semantic-hook/src"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
}

#[test]
fn codex_listfiles_source_dir_routes_to_ingest() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls crates/agent-semantic-hook/src",
                    "path": "crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls crates/agent-semantic-hook/src",
                    "path": "crates/agent-semantic-hook/src"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
    assert_eq!(
        decision.subject.command.as_deref(),
        Some("ls crates/agent-semantic-hook/src")
    );
}

#[test]
fn codex_listfiles_core_shape_preserves_cmd() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "list_files",
                    "cmd": "ls crates/agent-semantic-hook/src",
                    "path": "crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "list_files",
                    "cmd": "ls crates/agent-semantic-hook/src",
                    "path": "crates/agent-semantic-hook/src"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
    assert_eq!(
        decision.subject.command.as_deref(),
        Some("ls crates/agent-semantic-hook/src")
    );
}

#[test]
fn codex_read_core_shape_preserves_cmd() {
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
                        "type": "read",
                        "cmd": "sed -n '1,40p' crates/agent-semantic-hook/src/tool_action.rs",
                        "name": "tool_action.rs",
                        "path": "crates/agent-semantic-hook/src/tool_action.rs"
                    }
                }
            },
            "tool_input": {
                "item": {
                    "action": {
                        "type": "read",
                        "cmd": "sed -n '1,40p' crates/agent-semantic-hook/src/tool_action.rs",
                        "name": "tool_action.rs",
                        "path": "crates/agent-semantic-hook/src/tool_action.rs"
                    }
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.subject.command.as_deref(),
        Some("sed -n '1,40p' crates/agent-semantic-hook/src/tool_action.rs")
    );
}

#[test]
fn codex_listfiles_non_source_path_is_allowed() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls docs",
                    "path": "docs"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls docs",
                    "path": "docs"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
}

#[test]
fn codex_command_action_not_detected_in_arbitrary_payload() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "metadata_tool",
            "tool_name": "metadata_tool",
            "toolInput": {
                "metadata": {
                    "type": "listFiles",
                    "command": "not a command action",
                    "path": "crates/agent-semantic-hook/src"
                }
            },
            "tool_input": {
                "metadata": {
                    "type": "listFiles",
                    "command": "not a command action",
                    "path": "crates/agent-semantic-hook/src"
                }
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
}
