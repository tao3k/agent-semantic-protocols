use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python as registry;

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
