//! Terminal resident execution decisions that must never route back to a child.

use std::collections::BTreeMap;

pub(in crate::command::hook_runtime) fn append_terminal_execution_fields(
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
