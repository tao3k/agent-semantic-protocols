//! Pre-activation hook bootstrap decisions.

use super::hook_runtime_decision_render::emit_decision;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind, semantic_shell_tokens,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AspNoAgentSource {
    InheritedEnvironment,
    InlineCommandEnvironment,
}

impl AspNoAgentSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InheritedEnvironment => "inherited-environment",
            Self::InlineCommandEnvironment => "inline-command-environment",
        }
    }
}

pub(super) fn emit_asp_no_agent_receipt_if_requested(
    client: &str,
    event: &str,
    emit: &str,
    payload: &Value,
) -> Result<bool, String> {
    let Some(source) = asp_no_agent_source(payload, inherited_asp_no_agent()) else {
        return Ok(false);
    };
    emit_asp_no_agent_receipt(client, event, emit, payload, source)?;
    Ok(true)
}

fn inherited_asp_no_agent() -> bool {
    std::env::var_os("ASP_NO_AGENT").is_some_and(|value| value == OsStr::new("1"))
}

fn asp_no_agent_source(payload: &Value, inherited: bool) -> Option<AspNoAgentSource> {
    if inherited {
        return Some(AspNoAgentSource::InheritedEnvironment);
    }
    hook_payload_command(payload)
        .filter(|command| has_leading_asp_no_agent_assignment(command))
        .map(|_| AspNoAgentSource::InlineCommandEnvironment)
}

fn hook_payload_command(payload: &Value) -> Option<&str> {
    [
        "/tool_input/command",
        "/toolInput/command",
        "/input/command",
        "/command",
    ]
    .into_iter()
    .find_map(|pointer| payload.pointer(pointer).and_then(Value::as_str))
}

fn has_leading_asp_no_agent_assignment(command: &str) -> bool {
    semantic_shell_tokens(command)
        .first()
        .is_some_and(|token| token == "ASP_NO_AGENT=1")
}

fn emit_asp_no_agent_receipt(
    client: &str,
    event: &str,
    emit: &str,
    payload: &Value,
    source: AspNoAgentSource,
) -> Result<(), String> {
    let mut fields = BTreeMap::new();
    fields.insert(
        "bootstrapReceipt".to_string(),
        serde_json::json!({
            "schemaId": "asp.hook.bootstrap-receipt.v1",
            "status": "bypassed",
            "scope": "command",
            "source": source.as_str(),
            "activationLoad": "not-attempted",
            "agentDispatch": "bypassed"
        }),
    );
    emit_decision(
        emit,
        &HookDecision {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: client.to_string(),
            event: event.to_string(),
            decision: DecisionKind::Allow,
            reason_kind: ReasonKind::None,
            language_ids: Vec::new(),
            subject: DecisionSubject {
                tool_name: None,
                command: hook_payload_command(payload).map(str::to_string),
                paths: Vec::new(),
            },
            routes: Vec::new(),
            message: "ASP agent routing bypassed for this command.".to_string(),
            fields,
        },
    )
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_bootstrap.rs"]
mod tests;
