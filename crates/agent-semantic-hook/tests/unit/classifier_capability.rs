use super::enforce_registered_subagent_capability;
use crate::classifier::decision::allow;
use crate::protocol::{DecisionKind, DecisionSubject, ReasonKind};

fn subagent_payload(registration_verified: bool) -> serde_json::Value {
    serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
        "registration_verified": registration_verified,
    })
}

#[test]
fn subagent_without_registered_dispatch_capability_is_denied() {
    let decision = allow("codex", "pre-tool", DecisionSubject::default());
    let denied = enforce_registered_subagent_capability(decision, &subagent_payload(false));

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::SubagentReceiptRequired);
    assert_eq!(
        denied
            .fields
            .get("capabilityAdmission")
            .and_then(serde_json::Value::as_str),
        Some("registered-route-required")
    );
}

#[test]
fn role_only_dispatch_match_does_not_prove_registration() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "dispatchSatisfied".to_owned(),
        serde_json::Value::Bool(true),
    );
    let denied = enforce_registered_subagent_capability(decision, &subagent_payload(false));

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::SubagentReceiptRequired);
}

#[test]
fn host_registered_route_capability_is_admissible() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "dispatchSatisfied".to_owned(),
        serde_json::Value::Bool(true),
    );
    let allowed = enforce_registered_subagent_capability(decision, &subagent_payload(true));

    assert_eq!(allowed.decision, DecisionKind::Allow);
    assert_eq!(allowed.reason_kind, ReasonKind::None);
}
