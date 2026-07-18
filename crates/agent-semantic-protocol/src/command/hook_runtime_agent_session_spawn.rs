use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use std::collections::BTreeMap;

use super::AspSessionPolicy;

pub(super) fn resident_spawn_context_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    policy: &AspSessionPolicy,
) -> Option<HookDecision> {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(serde_json::Value::as_str)?;
    if !matches!(
        tool_name,
        "collaboration.spawn_agent" | "collaboration_v2.spawn_agent" | "spawn_agent"
    ) {
        return None;
    }
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))?;
    let agent_type = tool_input
        .get("agent_type")
        .or_else(|| tool_input.get("agentType"))
        .and_then(serde_json::Value::as_str)?;
    let (resident_name, expected_task_name) = configured_resident_identity(policy, agent_type)?;
    let task_name = tool_input
        .get("task_name")
        .or_else(|| tool_input.get("taskName"))
        .and_then(serde_json::Value::as_str);
    let fork_turns = tool_input
        .get("fork_turns")
        .or_else(|| tool_input.get("forkTurns"))
        .and_then(serde_json::Value::as_str);
    if task_name == Some(expected_task_name) && fork_turns == Some("none") {
        return None;
    }

    let canonical_target = format!("/root/{expected_task_name}");
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::SubagentReceiptRequired,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: Some(tool_name.to_string()),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: format!(
            "ASP requires configured resident `{resident_name}` to start as isolated canonical child `{canonical_target}` with agent_type={agent_type}, task_name={expected_task_name}, and fork_turns=none."
        ),
        fields: BTreeMap::from([
            (
                "requiredAction".to_string(),
                serde_json::json!("spawn-configured-resident-with-isolated-context"),
            ),
            (
                "residentChildName".to_string(),
                serde_json::json!(resident_name),
            ),
            (
                "targetAgentName".to_string(),
                serde_json::json!(expected_task_name),
            ),
            (
                "canonicalTarget".to_string(),
                serde_json::json!(canonical_target),
            ),
            (
                "managedAgentKind".to_string(),
                serde_json::json!(agent_type),
            ),
            (
                "requiredTaskName".to_string(),
                serde_json::json!(expected_task_name),
            ),
            ("requiredForkTurns".to_string(), serde_json::json!("none")),
            (
                "observedTaskName".to_string(),
                task_name.map_or(serde_json::Value::Null, serde_json::Value::from),
            ),
            (
                "observedForkTurns".to_string(),
                fork_turns.map_or(serde_json::Value::Null, serde_json::Value::from),
            ),
            (
                "residentContextPolicy".to_string(),
                serde_json::json!("profile-only-no-parent-turn-inheritance"),
            ),
        ]),
    })
}

fn configured_resident_identity<'a>(
    policy: &'a AspSessionPolicy,
    agent_type: &str,
) -> Option<(&'a str, &'a str)> {
    if policy.resident_codex_agent_name() == agent_type {
        return Some((
            policy.resident_child_name(),
            policy.resident_codex_agent_name(),
        ));
    }
    policy.execution_lanes.iter().find_map(|lane| {
        (lane.resident_codex_agent_name() == agent_type)
            .then(|| (lane.resident_child_name(), lane.resident_codex_agent_name()))
    })
}
