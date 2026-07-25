//! Shared hook-payload projections for read-only resident policy.

use serde_json::Value;

pub(super) fn string_field(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

pub(super) fn payload_message(payload: &Value) -> Option<String> {
    string_field(
        payload,
        &[
            "last_assistant_message",
            "lastAssistantMessage",
            "final_message",
            "finalMessage",
            "message",
            "content",
        ],
    )
}
