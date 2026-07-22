use super::hook_runtime_decision_render::{emit_decision, emit_hook_runtime_failure};
use super::string_field;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn emit_activation_load_failure(
    client: &str,
    event: &str,
    emit: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Result<(), String> {
    eprintln!(
        "[agent-semantic-hook] activation disabled for this hook event: {}: {error}",
        activation_path.display()
    );
    let decision = activation_load_failure_decision(client, event, activation_path, error, stdin);
    if let Some(decision) = decision {
        return emit_decision(emit, &decision);
    }
    emit_hook_runtime_failure(
        client,
        event,
        emit,
        &format!(
            "Semantic hook activation could not be loaded; allowing the activation recovery command: {error}"
        ),
    )
}

fn activation_load_failure_decision(
    client: &str,
    event: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Option<HookDecision> {
    let payload = serde_json::from_str::<serde_json::Value>(stdin).unwrap_or_default();
    let tool_name = string_field(&payload, &["tool_name", "toolName"]);
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("input"))
        .unwrap_or(&payload);
    let command = string_field(tool_input, &["cmd", "command", "script"]);
    if command
        .as_deref()
        .is_some_and(is_activation_recovery_command)
    {
        return None;
    }
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::ActivationUnavailable,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name,
            command,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: format!(
            "Semantic hook activation could not be loaded from {}; tool use is denied until activation is repaired: {error}",
            activation_path.display()
        ),
        fields: BTreeMap::new(),
    })
}

fn is_activation_recovery_command(command: &str) -> bool {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    tokens.windows(3).any(|window| {
        window[0]
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|binary| binary == "asp")
            && window[1] == "hook"
            && window[2] == "doctor"
    })
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_activation_recovery.rs"]
mod hook_runtime_activation_recovery_tests;
