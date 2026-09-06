//! Synthetic Codex rollout items projected into the Host PreToolUse boundary.
//!
//! Rollout logs are evidence for the public item shape only.  TestKit never
//! reads, stores, or depends on a user's rollout data: callers supply a
//! versioned synthetic inference-response fixture instead.

use serde_json::Value;
use serde_json::json;

use crate::HookTestKitError;

/// Stable context supplied by the Host when a synthetic function call reaches
/// the PreToolUse hook.  The command itself always comes from the rollout
/// `function_call.arguments`, never from this context.
#[derive(Clone, Debug)]
pub struct CodexPreToolContext<'a> {
    pub cwd: &'a str,
    pub model: &'a str,
    pub permission_mode: &'a str,
    pub session_id: &'a str,
    pub transcript_path: Option<&'a str>,
    pub turn_id: &'a str,
}

/// Projects each canonical Codex `exec_command` function call in a synthetic
/// inference response into the exact PreToolUse command-input shape.
///
/// Codex rollout responses carry calls in `output_items`; the hook itself sees
/// the later, separate `PreToolUse` envelope.  Keeping this translation in the
/// TestKit prevents hand-written Hook payloads from drifting away from Codex's
/// actual function-call schema.
pub fn project_codex_exec_command_calls(
    inference_response: &Value,
    context: &CodexPreToolContext<'_>,
) -> Result<Vec<Value>, HookTestKitError> {
    let items = inference_response
        .get("output_items")
        .and_then(Value::as_array)
        .ok_or_else(|| projection_error("inference response requires output_items array"))?;

    items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter(|item| item.get("name").and_then(Value::as_str) == Some("exec_command"))
        .map(|item| project_exec_command_item(item, context))
        .collect()
}

fn project_exec_command_item(
    item: &Value,
    context: &CodexPreToolContext<'_>,
) -> Result<Value, HookTestKitError> {
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| projection_error("exec_command function_call requires nonempty call_id"))?;
    let arguments = item
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or_else(|| projection_error("exec_command function_call requires string arguments"))?;
    let arguments: Value = serde_json::from_str(arguments)
        .map_err(|error| projection_error(format!("decode exec_command arguments: {error}")))?;
    let command = arguments
        .get("cmd")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| projection_error("exec_command arguments requires nonempty cmd"))?;

    Ok(json!({
        "cwd": context.cwd,
        "hook_event_name": "PreToolUse",
        "model": context.model,
        "permission_mode": context.permission_mode,
        "session_id": context.session_id,
        "tool_input": { "command": command },
        "tool_name": "Bash",
        "tool_use_id": call_id,
        "transcript_path": context.transcript_path,
        "turn_id": context.turn_id,
    }))
}

fn projection_error(message: impl Into<String>) -> HookTestKitError {
    HookTestKitError::Scenario {
        scenario_id: "codex-rollout-projection".to_owned(),
        message: message.into(),
    }
}
