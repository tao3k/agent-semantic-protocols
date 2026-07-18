//! Owns the explicit current-session transport for parser-classified ASP exploration.

use std::collections::BTreeMap;

use super::{
    AspSessionPolicy, DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, agent_session_route_fields, append_asp_command_intent_fields,
    append_resident_agent_fields, resident_agent_host_action, string_field,
};

pub(super) fn missing_resident_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: Option<&str>,
    root_session_id: Option<String>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("start-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    append_bootstrap_fields(&mut fields, resident_child_name);
    if let Some(root_session_id) = root_session_id {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id),
        );
    }
    if let Some(command) = command {
        append_asp_command_intent_fields(&mut fields, command, asp_session_policy);
    }
    let language_ids = fields
        .get("languageId")
        .and_then(serde_json::Value::as_str)
        .map(|language_id| vec![language_id.to_string()])
        .unwrap_or_default();
    let bootstrap_command = format!("asp agent session bootstrap --name {resident_child_name}");
    let host_action = resident_agent_host_action(platform, asp_session_policy, false);
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::AspReasoningRouted,
        language_ids,
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: command.map(str::to_string),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: format!(
            "ASP resident lifecycle v1 requires a verified typed resident.\nRun `{bootstrap_command}` and choose one structured menu option. {host_action} If the host cannot provide the configured agent type or a verified live binding, this resident command remains locally blocked while unrelated Codex tools remain available."
        ),
        fields,
    }
}

fn append_bootstrap_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    resident_child_name: &str,
) {
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("session-start-reminder".to_string()),
    );
    for field_name in [
        "agentSessionBootstrapGuideCommand",
        "agentSessionBootstrapCommand",
    ] {
        fields.insert(
            field_name.to_string(),
            serde_json::Value::String(format!(
                "asp agent session bootstrap --name {resident_child_name}"
            )),
        );
    }
}
