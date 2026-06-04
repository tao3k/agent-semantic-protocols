use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python as registry;

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
