use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodexNativeSubagentEventKind {
    Start,
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CodexNativeSubagentEvent {
    pub(crate) kind: CodexNativeSubagentEventKind,
    pub(crate) root_session_id: String,
    pub(crate) agent_id: String,
    pub(crate) agent_type: String,
    pub(crate) model: String,
    pub(crate) permission_mode: String,
}

impl CodexNativeSubagentEvent {
    #[must_use]
    pub(crate) fn message_target_id(&self) -> &str {
        &self.agent_id
    }
}

pub(crate) fn parse_subagent_event(
    payload: &Value,
) -> Result<Option<CodexNativeSubagentEvent>, String> {
    let Some(hook_event_name) = payload.get("hook_event_name").and_then(Value::as_str) else {
        return Ok(None);
    };
    let kind = match hook_event_name {
        "SubagentStart" => CodexNativeSubagentEventKind::Start,
        "SubagentStop" => CodexNativeSubagentEventKind::Stop,
        _ => return Ok(None),
    };
    Ok(Some(CodexNativeSubagentEvent {
        kind,
        root_session_id: required_string(payload, "session_id")?,
        agent_id: required_string(payload, "agent_id")?,
        agent_type: required_string(payload, "agent_type")?,
        model: required_string(payload, "model")?,
        permission_mode: required_string(payload, "permission_mode")?,
    }))
}

fn required_string(payload: &Value, field: &str) -> Result<String, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Codex native subagent event missing `{field}`"))
}
