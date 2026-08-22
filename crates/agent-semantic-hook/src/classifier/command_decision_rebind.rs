//! Rebinds config-compiled shell decisions to exact one-shot Host envelopes.

use serde_json::Value;

use crate::HookDecision;

use super::DirectReadSourceKey;
use super::core::{
    ShellReadSourceKey, collect_payload_tool_actions, direct_read_source_key_from_actions,
    enforce_org_choice_plane_boundary, resolve_dispatch_decision,
    shell_read_source_keys_from_actions, with_action_receipt_fields,
};

/// Normalized lookup key for a one-action shell command decision shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellCommandKey {
    pub command: String,
    pub command_tokens: Vec<String>,
    pub paths: Vec<String>,
    pub tool_name: String,
}

/// All declarative matcher keys derived from one canonical Host normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookMatcherKeys {
    pub direct_read: Option<DirectReadSourceKey>,
    pub shell_reads: Vec<ShellReadSourceKey>,
    pub shell_commands: Vec<ShellCommandKey>,
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
            })
        })
        .collect()
}

/// Project direct-read and shell matcher keys with one parser-owned pass.
pub fn hook_matcher_keys(payload: &Value) -> HookMatcherKeys {
    let actions = collect_payload_tool_actions(payload);
    let direct_read = direct_read_source_key_from_actions(&actions);
    let (shell_reads, shell_commands) = if direct_read.is_some() {
        (Vec::new(), Vec::new())
    } else {
        (
            shell_read_source_keys_from_actions(&actions),
            shell_command_keys_from_actions(&actions),
        )
    };
    HookMatcherKeys {
        direct_read,
        shell_reads,
        shell_commands,
    }
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
    enforce_org_choice_plane_boundary(resolve_dispatch_decision(decision, payload))
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
    enforce_org_choice_plane_boundary(resolve_dispatch_decision(decision, payload))
}
