use serde_json::{Map, Value};

pub(super) fn payload_indicates_subagent_context(payload: &Value) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    object_has_true_bool(
        object,
        &[
            "is_subagent",
            "isSubagent",
            "subagent",
            "is_child_agent",
            "isChildAgent",
        ],
    ) || object_has_present_value(
        object,
        &[
            "parent_agent_id",
            "parentAgentId",
            "parent_session_id",
            "parentSessionId",
            "parent_thread_id",
            "parentThreadId",
        ],
    ) || object_has_subagent_text(
        object,
        &[
            "agent_kind",
            "agentKind",
            "thread_kind",
            "threadKind",
            "context_kind",
            "contextKind",
        ],
    ) || ["agent", "thread", "context", "_meta"]
        .iter()
        .filter_map(|key| object.get(*key))
        .any(payload_indicates_subagent_context)
}

fn object_has_true_bool(object: &Map<String, Value>, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| object.get(*key).and_then(Value::as_bool) == Some(true))
}

fn object_has_present_value(object: &Map<String, Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        object.get(*key).is_some_and(|value| match value {
            Value::Null => false,
            Value::String(text) => !text.trim().is_empty(),
            _ => true,
        })
    })
}

fn object_has_subagent_text(object: &Map<String, Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(|text| {
                let normalized = text.to_ascii_lowercase();
                normalized.contains("subagent") || normalized.contains("child")
            })
    })
}
