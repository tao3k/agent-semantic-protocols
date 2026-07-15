//! Owns the explicit current-session transport for parser-classified ASP exploration.

use std::collections::BTreeMap;

use super::{
    AspExplorationCommand, AspSessionPolicy, DecisionKind, DecisionSubject,
    HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HookClientExecutionTransport, HookDecision, ReasonKind, agent_session_route_fields,
    append_asp_command_intent_fields, append_resident_agent_fields, command_sha256,
    resident_agent_host_action, shell_like_tokens, string_field,
};

pub(super) fn missing_resident_asp_explore_decision(
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
            "ASP resident child lifecycle is preferred.\nRun `{bootstrap_command}` and choose one structured menu option. {host_action} When the host cannot express the configured agent type, rerun only the exact parser-owned ASP command with `ASP_INLINE_PARSER_FALLBACK=1`; this selects the degraded current-session transport without authorizing raw source fallback."
        ),
        fields,
    }
}

pub(super) fn current_session_exploration_fallback_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    command: &str,
    invocation: AspExplorationCommand,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let mut fields = fallback_receipt_fields(command);
    append_asp_command_intent_fields(&mut fields, command, asp_session_policy);
    append_payload_identity_fields(&mut fields, payload);
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: invocation.language_id.into_iter().collect(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: Some(command.to_string()),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: "ASP allowed an explicitly selected parser-owned exploration command in the current session because no profile-valid resident child is available.".to_string(),
        fields,
    }
}

pub(super) fn command_enables_inline_parser_fallback(command: &str) -> bool {
    shell_like_tokens(command)
        .iter()
        .any(|token| token == "ASP_INLINE_PARSER_FALLBACK=1")
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
    fields.insert(
        "inlineParserFallbackAvailable".to_string(),
        serde_json::Value::Bool(true),
    );
    fields.insert(
        "inlineParserFallbackOptIn".to_string(),
        serde_json::Value::String("ASP_INLINE_PARSER_FALLBACK=1".to_string()),
    );
    fields.insert(
        "inlineParserFallbackPolicy".to_string(),
        serde_json::Value::String("exact-parser-owned-command-only".to_string()),
    );
}

fn fallback_receipt_fields(command: &str) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        string_field_value("executionLane", "asp-explore"),
        string_field_value(
            "executionTransport",
            HookClientExecutionTransport::CurrentSession.as_str(),
        ),
        string_field_value("executionReceiptKind", "asp-search-subagent"),
        string_field_value("executionCommandDigest", &command_sha256(command)),
        string_field_value("agentSessionAction", "inline-parser-fallback"),
        ("residentChild".to_string(), serde_json::Value::Bool(false)),
        ("degraded".to_string(), serde_json::Value::Bool(true)),
        string_field_value("fallbackReason", "no-profile-valid-resident-child"),
        string_field_value(
            "fallbackPolicy",
            "explicit-opt-in-exact-parser-command-only",
        ),
    ])
}

fn append_payload_identity_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    payload: &serde_json::Value,
) {
    if let Some(root_session_id) = string_field(
        payload,
        &[
            "root_session_id",
            "rootSessionId",
            "session_id",
            "sessionId",
        ],
    ) {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id),
        );
    }
    if let Some(cwd) = string_field(payload, &["cwd"]) {
        fields.insert("cwd".to_string(), serde_json::Value::String(cwd));
    }
}

fn string_field_value(name: &str, value: &str) -> (String, serde_json::Value) {
    (
        name.to_string(),
        serde_json::Value::String(value.to_string()),
    )
}
