use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python as registry;

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
