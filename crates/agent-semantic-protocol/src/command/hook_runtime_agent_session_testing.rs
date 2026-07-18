//! Configured hook-selected resident execution lane decisions.

use std::collections::BTreeMap;

use agent_semantic_config::HookClientExecutionTransport;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use sha2::{Digest, Sha256};

use super::ResidentExecutionLane;

pub(super) fn resident_execution_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    lane: &ResidentExecutionLane,
) -> HookDecision {
    match lane.transport() {
        HookClientExecutionTransport::CurrentSession => {
            current_session_decision(platform, event, payload, command, lane)
        }
        HookClientExecutionTransport::ResidentAgent => {
            resident_agent_decision(platform, event, payload, command, lane)
        }
    }
}

fn current_session_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    lane: &ResidentExecutionLane,
) -> HookDecision {
    decision(
        platform,
        event,
        payload,
        command,
        lane,
        DecisionKind::Allow,
        ReasonKind::None,
        "current-session",
        "ASP allowed an explicitly configured current-session execution lane command.",
    )
}

fn resident_agent_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    lane: &ResidentExecutionLane,
) -> HookDecision {
    decision(
        platform,
        event,
        payload,
        command,
        lane,
        DecisionKind::Deny,
        ReasonKind::SubagentReceiptRequired,
        "resident-agent",
        &format!(
            "ASP denied this command in the main Agent. Route it exactly through hook-selected lane `{}` to configured resident `{}` at `/root/{}` and return its digest-bound receipt.",
            lane.name(),
            lane.resident_child_name(),
            lane.resident_codex_agent_name()
        ),
    )
}

fn decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    lane: &ResidentExecutionLane,
    decision: DecisionKind,
    reason_kind: ReasonKind,
    transport: &str,
    message: &str,
) -> HookDecision {
    let mut fields = BTreeMap::from([
        ("executionLane".to_string(), json_string(lane.name())),
        ("executionTransport".to_string(), json_string(transport)),
        (
            "executionReceiptKind".to_string(),
            json_string(lane.receipt_kind()),
        ),
        (
            "executionCommandDigest".to_string(),
            json_string(&command_sha256(command)),
        ),
        ("blockedCommandClass".to_string(), json_string(lane.name())),
    ]);
    if transport == "resident-agent" {
        fields.extend([
            (
                "residentChildName".to_string(),
                json_string(lane.resident_child_name()),
            ),
            (
                "targetAgentName".to_string(),
                json_string(lane.resident_codex_agent_name()),
            ),
            (
                "targetAgentRole".to_string(),
                json_string(lane.resident_agent_role()),
            ),
            (
                "canonicalTarget".to_string(),
                json_string(&format!("/root/{}", lane.resident_codex_agent_name())),
            ),
            (
                "requiredAction".to_string(),
                json_string("route-exact-command-to-hook-selected-resident"),
            ),
        ]);
    }
    if let Some(root_session_id) = string_field(
        payload,
        &[
            "root_session_id",
            "rootSessionId",
            "session_id",
            "sessionId",
        ],
    ) {
        fields.insert("rootSessionId".to_string(), json_string(&root_session_id));
    }
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision,
        reason_kind,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: Some(command.to_string()),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: message.to_string(),
        fields,
    }
}

fn json_string(value: &str) -> serde_json::Value {
    serde_json::Value::String(value.to_string())
}

fn command_sha256(command: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(command.as_bytes()))
}

fn string_field(payload: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}
