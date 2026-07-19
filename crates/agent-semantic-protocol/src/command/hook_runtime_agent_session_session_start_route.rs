use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};

use super::hook_runtime_agent_session_pane::{
    agent_session_route_fields, render_agent_session_template,
};
use super::hook_runtime_agent_session_payload::string_field;
use super::hook_runtime_agent_session_profile::{
    append_resident_agent_fields, resident_child_create_action,
};
use super::{AspSessionPolicy, template_value};

pub(super) fn registry_lookup_for_route_child<T>(
    result: Result<Option<T>, String>,
    rollout_direct_resident_child: bool,
) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(value),
        Err(error)
            if rollout_direct_resident_child && registry_unavailable_for_route_child(&error) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn registry_unavailable_for_route_child(error: &str) -> bool {
    error.contains("failed to open Turso agent session registry")
        || error.contains("database is locked")
        || error.contains("locking error")
        || error.contains("failed locking file")
}

pub(super) fn session_start_reuse_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    session: &AgentSessionRecord,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("reuse-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    fields.insert(
        "rootSessionId".to_string(),
        serde_json::Value::String(session.root_session_id.clone()),
    );
    fields.insert(
        "childSessionId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "agentSessionExistingChildId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "childSessionName".to_string(),
        serde_json::Value::String(session.name.clone()),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String("enter-bootstrap-pane-for-existing-child".to_string()),
    );
    let message = render_agent_session_template(
        asp_session_policy.messages.session_start_reuse.as_deref(),
        &[
            template_value("residentChildName", resident_child_name),
            template_value("childSessionId", &session.session_id),
            template_value("rootSessionId", &session.root_session_id),
        ],
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

pub(super) fn session_start_resume_existing_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    session: &AgentSessionRecord,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields =
        agent_session_route_fields("resume-existing-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    fields.insert(
        "rootSessionId".to_string(),
        serde_json::Value::String(session.root_session_id.clone()),
    );
    fields.insert(
        "childSessionId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "agentSessionResumeId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "childSessionName".to_string(),
        serde_json::Value::String(session.name.clone()),
    );
    fields.insert(
        "childSessionStatus".to_string(),
        serde_json::Value::String(session.status.clone()),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String("enter-bootstrap-pane-for-existing-child".to_string()),
    );
    let message = format!(
        "Existing resident {resident_child_name} child session {} is registered with status {}. Enter the resident-child choice pane and let it recover or replace that child; do not create a generic replacement outside the pane.",
        session.session_id, session.status
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

pub(super) fn session_start_bootstrap_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    root_session_id: Option<String>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields =
        agent_session_route_fields("enter-resident-child-bootstrap-pane", resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("session-start-reminder".to_string()),
    );
    fields.insert(
        "agentSessionBootstrapGuideCommand".to_string(),
        serde_json::Value::String(format!(
            "asp agent session bootstrap --name {resident_child_name}"
        )),
    );
    if let Some(root_session_id) = root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
    let create_action = resident_child_create_action(platform, asp_session_policy);
    let message = render_agent_session_template(
        asp_session_policy
            .messages
            .session_start_bootstrap
            .as_deref(),
        &[
            template_value("residentChildName", resident_child_name),
            template_value(
                "residentCodexAgentName",
                asp_session_policy.resident_codex_agent_name(),
            ),
            template_value("createAction", &create_action),
            template_value("rootSessionId", root_session_id.as_deref().unwrap_or("")),
        ],
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}
