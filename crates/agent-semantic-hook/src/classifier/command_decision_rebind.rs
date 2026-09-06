//! Rebinds config-compiled shell decisions to exact one-shot Host envelopes.

use serde_json::Value;

use crate::HookDecision;

use super::core::ShellReadSourceKey;
use super::core::collect_payload_tool_actions;
use super::core::resolve_dispatch_decision;
use super::core::shell_read_source_keys_from_actions;
use super::core::with_action_receipt_fields;

/// Normalized lookup key for a one-action shell command decision shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellCommandKey {
    pub command: String,
    pub command_tokens: Vec<String>,
    pub paths: Vec<String>,
    pub tool_name: String,
    pub has_declared_filesystem_access: bool,
}

fn shell_command_keys_from_actions(
    actions: &[crate::tool_action::ToolAction],
) -> Vec<ShellCommandKey> {
    actions
        .iter()
        .filter(|action| {
            action.surface == crate::tool_action::ToolSurface::CodexShell
                && action.operation == crate::tool_action::OperationIntent::ShellCommand
        })
        .filter_map(|action| {
            Some(ShellCommandKey {
                command: action.command.clone()?,
                command_tokens: action.command_tokens()?.into_owned(),
                paths: action.paths.clone(),
                tool_name: action.tool_name.clone(),
                // `paths` is projected by the single canonical shell parse
                // while normalizing the ToolAction. Do not derive the full
                // AgentAction here: that would parse the shell AST again on
                // the Host's latency-sensitive decision path.
                has_declared_filesystem_access: action.has_declared_filesystem_access,
            })
        })
        .collect()
}

/// Return every parser-owned shell action key in Host execution order.
pub fn shell_command_keys(payload: &Value) -> Vec<ShellCommandKey> {
    let actions = collect_payload_tool_actions(payload);
    shell_command_keys_from_actions(&actions)
}

/// Return the normalized command key for a one-action shell envelope.
pub fn shell_command_key(payload: &Value) -> Option<ShellCommandKey> {
    let mut keys = shell_command_keys(payload).into_iter();
    let key = keys.next()?;
    keys.next().is_none().then_some(key)
}

/// Rebind a config-compiled command decision to the exact Host payload without
/// re-running the Policy Kernel.
pub fn rebind_command_decision_to_payload(decision: HookDecision, payload: &Value) -> HookDecision {
    let actions = collect_payload_tool_actions(payload);
    let mut decision = with_action_receipt_fields(decision, payload, &actions);
    if let Some(key) = shell_command_key(payload) {
        decision.subject.command = Some(key.command.clone());
        decision.subject.tool_name = Some(key.tool_name);
    }
    resolve_dispatch_decision(decision, payload)
}

/// Rebind a decision using command keys already normalized by the Hook hot
/// path, avoiding a second shell AST parse solely for receipt materialization.
pub fn rebind_command_decision_to_payload_with_keys(
    mut decision: HookDecision,
    payload: &Value,
    keys: &[ShellCommandKey],
) -> HookDecision {
    let payload_keys = payload
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    decision.fields.insert(
        "hookPayloadKeys".to_owned(),
        Value::Array(payload_keys.into_iter().map(Value::String).collect()),
    );
    decision.fields.insert(
        "normalizedActions".to_owned(),
        Value::Array(
            keys.iter()
                .map(|key| {
                    serde_json::json!({
                        "toolName": key.tool_name,
                        "toolSurface": "shell-command",
                        "operationIntent": "shell-command",
                        "paths": key.paths,
                    })
                })
                .collect(),
        ),
    );
    resolve_dispatch_decision(decision, payload)
}
