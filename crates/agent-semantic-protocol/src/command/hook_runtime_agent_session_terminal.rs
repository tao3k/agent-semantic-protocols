//! Terminal resident execution decisions that must never route back to a child.

use std::collections::BTreeMap;

use super::hook_runtime_agent_session_command::command_contains_asp_binary;

pub(super) fn proven_resident_parser_command_is_terminal(
    commands: &[String],
    resident_identity_proven: bool,
) -> bool {
    resident_identity_proven
        && commands
            .iter()
            .any(|command| command_contains_asp_binary(command))
}

pub(super) fn resident_dispatch_wrapper_is_terminal(commands: &[String]) -> bool {
    commands.iter().any(|command| {
        command_contains_asp_binary(command) && command.contains("agent session dispatch-execute")
    })
}

pub(super) fn append_terminal_execution_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    action: &str,
) {
    if !matches!(
        action,
        "active-resident-child" | "active-hook-selected-resident" | "resident-command-bridge"
    ) {
        return;
    }
    let bridge = action == "resident-command-bridge";
    if bridge {
        fields.insert(
            "executionLane".to_string(),
            serde_json::Value::String("hook-selected-resident".to_string()),
        );
    }
    fields.extend([
        (
            "executionTransport".to_string(),
            serde_json::Value::String(if bridge {
                "resident-command-bridge".to_string()
            } else {
                "resident-child-terminal".to_string()
            }),
        ),
        ("routingTerminal".to_string(), serde_json::Value::Bool(true)),
        (
            "redispatchAllowed".to_string(),
            serde_json::Value::Bool(false),
        ),
        (
            "executionReceiptKind".to_string(),
            serde_json::Value::String("resident-command-dispatch-receipt".to_string()),
        ),
    ]);
}
