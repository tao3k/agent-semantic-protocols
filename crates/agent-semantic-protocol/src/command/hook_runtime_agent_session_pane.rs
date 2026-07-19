use super::append_terminal_execution_fields;
use super::hook_runtime_agent_session_payload::{payload_command_strings, string_field};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use std::collections::BTreeMap;

pub(super) fn render_agent_session_template(
    template: Option<&str>,
    values: &[(&'static str, String)],
) -> String {
    let mut rendered = template
        .unwrap_or("ASP agent session routing template missing. Run `asp sync` and retry.")
        .to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered.trim().to_string()
}

pub(super) fn agent_session_route_fields(
    action: &str,
    resident_child_name: &str,
) -> BTreeMap<String, serde_json::Value> {
    let mut fields = BTreeMap::from([
        (
            "agentSessionRoute".to_string(),
            serde_json::Value::String(resident_child_name.to_string()),
        ),
        (
            "agentSessionLifecycle".to_string(),
            serde_json::Value::String("resident".to_string()),
        ),
        (
            "agentSessionLoopCommand".to_string(),
            serde_json::Value::String(format!(
                "asp agent session bootstrap --name {resident_child_name}"
            )),
        ),
        (
            "agentSessionTimeoutPolicy".to_string(),
            serde_json::Value::String("timeout-is-not-duplicate-worker-trigger".to_string()),
        ),
        (
            "agentSessionAction".to_string(),
            serde_json::Value::String(action.to_string()),
        ),
        (
            "agentSessionSpawnPolicy".to_string(),
            serde_json::Value::String("registered-profile-valid-child-only".to_string()),
        ),
        (
            "agentSessionValidationPolicy".to_string(),
            serde_json::Value::String("register-hard-validates-profile".to_string()),
        ),
        (
            "agentSessionInvalidChildAction".to_string(),
            serde_json::Value::String(
                "close-native-subagent-or-archive-temporary-thread-and-create-configured-child"
                    .to_string(),
            ),
        ),
        (
            "agentSessionDuplicatePolicy".to_string(),
            serde_json::Value::String(
                "one-active-resident-child-per-root-session-and-name".to_string(),
            ),
        ),
    ]);
    append_agent_session_recovery_action_fields(
        &mut fields,
        action,
        resident_child_name,
        resident_child_name,
    );
    fields
}

pub(super) fn append_agent_session_recovery_action_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    action: &str,
    resident_child_name: &str,
    resident_role: &str,
) {
    let (required_action, next_action, completion_receipt) = match action {
        "start-resident-child" => (
            format!("enter-{resident_child_name}-choice-pane"),
            "choose-one-bootstrap-pane-option".to_string(),
            format!("{resident_child_name}-choice-pane-receipt"),
        ),
        "reuse-resident-child" | "resume-resident-child" => (
            format!("use-existing-{resident_child_name}-through-pane"),
            "enter-bootstrap-pane-if-transport-is-not-ready".to_string(),
            format!("{resident_child_name}-child-command"),
        ),
        _ => (
            format!("enter-{resident_child_name}-choice-pane"),
            "choose-one-bootstrap-pane-option".to_string(),
            format!("{resident_child_name}-choice-pane-receipt"),
        ),
    };

    fields.insert(
        "requiredAction".to_string(),
        serde_json::Value::String(required_action),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String(next_action),
    );
    fields.insert(
        "targetAgentName".to_string(),
        serde_json::Value::String(resident_child_name.to_string()),
    );
    fields.insert(
        "targetAgentRole".to_string(),
        serde_json::Value::String(resident_role.to_string()),
    );
    fields.insert(
        "forbiddenUntilResolved".to_string(),
        serde_json::Value::String("raw-source-fallback".to_string()),
    );
    fields.insert(
        "completionReceipt".to_string(),
        serde_json::Value::String(completion_receipt),
    );
}

pub(super) fn agent_session_allow_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    action: &str,
    message: &str,
) -> HookDecision {
    let mut fields = BTreeMap::from([(
        "agentSessionAction".to_string(),
        serde_json::Value::String(action.to_string()),
    )]);
    append_terminal_execution_fields(&mut fields, action);
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: payload_command_strings(payload).into_iter().next(),
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message: message.to_string(),
        fields,
    }
}
