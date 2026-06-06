//! Shared hook decision constructors for classifier routes.

use serde_json::Value;

use crate::{
    DecisionKind, DecisionRoute, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, ToolAction,
};

pub(super) fn allow(platform: &str, event: &str, subject: DecisionSubject) -> HookDecision {
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
        subject,
        routes: Vec::new(),
        message: "Allowed by semantic agent hook runtime.".to_string(),
        fields: std::collections::BTreeMap::new(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn deny_for_action(
    platform: &str,
    event: &str,
    reason_kind: ReasonKind,
    action: &ToolAction,
    language_ids: Vec<String>,
    subject: DecisionSubject,
    routes: Vec<DecisionRoute>,
    message: String,
) -> HookDecision {
    let mut decision = deny(
        platform,
        event,
        reason_kind,
        language_ids,
        subject,
        routes,
        message,
    );
    decision.fields.insert(
        "toolSurface".to_string(),
        Value::String(action.surface.as_str().to_string()),
    );
    decision.fields.insert(
        "operationIntent".to_string(),
        Value::String(action.operation.as_str().to_string()),
    );
    decision
}

fn deny(
    platform: &str,
    event: &str,
    reason_kind: ReasonKind,
    language_ids: Vec<String>,
    subject: DecisionSubject,
    routes: Vec<DecisionRoute>,
    message: String,
) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind,
        language_ids,
        subject,
        routes,
        message,
        fields: std::collections::BTreeMap::new(),
    }
}
