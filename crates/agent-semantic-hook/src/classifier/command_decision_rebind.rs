//! Rebinds config-compiled shell decisions to exact one-shot Host envelopes.

use serde_json::Value;

use crate::HookDecision;

use super::core::{
    collect_payload_tool_actions, enforce_org_choice_plane_boundary, resolve_dispatch_decision,
    with_action_receipt_fields,
};

/// Normalized lookup key for a one-action shell command decision shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellCommandKey {
    pub command: String,
    pub command_tokens: Vec<String>,
    pub paths: Vec<String>,
    pub tool_name: String,
}

/// Return the normalized command key for a one-action shell envelope.
pub fn shell_command_key(payload: &Value) -> Option<ShellCommandKey> {
    let actions = collect_payload_tool_actions(payload);
    let mut relevant = actions.iter().filter(|action| {
        action.surface == crate::tool_action::ToolSurface::CodexShell
            && action.operation == crate::tool_action::OperationIntent::ShellCommand
    });
    let action = relevant.next()?;
    if relevant.next().is_some() {
        return None;
    }
    Some(ShellCommandKey {
        command: action.command.clone()?,
        command_tokens: action.command_tokens()?.into_owned(),
        paths: action.paths.clone(),
        tool_name: action.tool_name.clone(),
    })
}

/// Rebind a config-compiled command decision to the exact Host payload without
/// re-running the Policy Kernel.
pub fn rebind_command_decision_to_payload(decision: HookDecision, payload: &Value) -> HookDecision {
    let actions = collect_payload_tool_actions(payload);
    let mut decision = with_action_receipt_fields(decision, payload, &actions);
    if let Some(key) = shell_command_key(payload) {
        decision.subject.command = Some(key.command.clone());
        decision.subject.tool_name = Some(key.tool_name);
        if decision.reason_kind == crate::ReasonKind::SubagentReceiptRequired
            && !decision.message.contains(&key.command)
        {
            decision
                .message
                .push_str(&format!("\nDenied command: `{}`.", key.command));
        }
    }
    enforce_org_choice_plane_boundary(resolve_dispatch_decision(decision, payload))
}
