use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::registry;

#[test]
fn codex_listfiles_absolute_paths_respect_configured_project_root() {
    let mut registry = registry();
    registry.project_root = "/tmp/asp-workspace".to_string();

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/crates/agent-semantic-hook/src"
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace-shadow/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace-shadow/crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace-shadow/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace-shadow/crates/agent-semantic-hook/src"
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
}

#[test]
fn codex_listfiles_absolute_paths_respect_current_dir_project_root() {
    let mut registry = registry();
    registry.project_root = ".".to_string();
    let current_root = std::env::current_dir().unwrap();
    let current_source_path = current_root
        .join("crates/agent-semantic-hook/src")
        .to_string_lossy()
        .to_string();
    let current_source_command = format!("ls {current_source_path}");

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": current_source_command,
                    "path": current_source_path
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": current_source_command,
                    "path": current_source_path
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);

    let current_shadow_path = format!(
        "{}-shadow/crates/agent-semantic-hook/src",
        current_root.to_string_lossy()
    );
    let current_shadow_command = format!("ls {current_shadow_path}");
    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": current_shadow_command,
                    "path": current_shadow_path
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": current_shadow_command,
                    "path": current_shadow_path
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
}

#[test]
fn codex_listfiles_traversal_paths_are_normalized_before_source_matching() {
    let mut registry = registry();
    registry.project_root = "/tmp/asp-workspace".to_string();

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/../asp-workspace/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/../asp-workspace/crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/../asp-workspace/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/../asp-workspace/crates/agent-semantic-hook/src"
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/../asp-workspace-shadow/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/../asp-workspace-shadow/crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "listFiles",
                    "command": "ls /tmp/asp-workspace/../asp-workspace-shadow/crates/agent-semantic-hook/src",
                    "path": "/tmp/asp-workspace/../asp-workspace-shadow/crates/agent-semantic-hook/src"
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
}
