use super::{
    HookClassificationRequest, classify_hook_with_config, enforce_registered_subagent_capability,
    resolve_dispatch_decision,
};
use crate::classifier::decision::allow;
use crate::protocol::{DecisionKind, DecisionSubject, ReasonKind};

fn subagent_payload(registration_verified: bool) -> serde_json::Value {
    serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
        "registration_verified": registration_verified,
        "registered_agent_name": "asp_explorer",
        "registered_denied_actions": ["edit"],
        "registered_allowed_rule_intents": ["reasoning-search"],
    })
}

#[test]
fn explicit_no_agent_bypass_precedes_registered_subagent_admission() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "bypassOwner".to_owned(),
        serde_json::Value::String("hook-matcher".to_owned()),
    );
    decision.fields.insert(
        "bypassScope".to_owned(),
        serde_json::Value::String("host-policy".to_owned()),
    );

    let allowed = enforce_registered_subagent_capability(decision, &subagent_payload(false), &[]);

    assert_eq!(allowed.decision, DecisionKind::Allow);
    assert_eq!(allowed.reason_kind, ReasonKind::None);
}

#[test]
fn subagent_without_registered_dispatch_capability_is_denied() {
    let decision = allow("codex", "pre-tool", DecisionSubject::default());
    let denied = enforce_registered_subagent_capability(decision, &subagent_payload(false), &[]);

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
    decision.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("reasoning-search".to_owned()),
    );
    let denied = enforce_registered_subagent_capability(decision, &subagent_payload(false), &[]);

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
    decision.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("reasoning-search".to_owned()),
    );
    let allowed = enforce_registered_subagent_capability(decision, &subagent_payload(true), &[]);

    assert_eq!(allowed.decision, DecisionKind::Allow);
    assert_eq!(allowed.reason_kind, ReasonKind::None);
}

#[test]
fn verified_registered_intent_makes_dispatch_idempotent_without_symbol_relookup() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.decision = DecisionKind::Deny;
    decision.reason_kind = ReasonKind::SubagentReceiptRequired;
    decision.fields.insert(
        "targetAgent".to_owned(),
        serde_json::Value::String("asp_explorer".to_owned()),
    );
    decision.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("reasoning-search".to_owned()),
    );

    let allowed = resolve_dispatch_decision(decision, &subagent_payload(true));

    assert_eq!(allowed.decision, DecisionKind::Allow);
    assert_eq!(allowed.reason_kind, ReasonKind::None);
    assert_eq!(allowed.fields["dispatchSatisfied"], true);
    assert_eq!(
        allowed.fields["dispatchAdmission"],
        "verified-registration-intent"
    );
}

#[test]
fn unrelated_registered_intent_cannot_satisfy_explorer_dispatch() {
    let mut payload = subagent_payload(true);
    payload["registered_allowed_rule_intents"] = serde_json::json!(["test-build-command"]);
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.decision = DecisionKind::Deny;
    decision.reason_kind = ReasonKind::SubagentReceiptRequired;
    decision.fields.insert(
        "targetAgent".to_owned(),
        serde_json::Value::String("asp_explorer".to_owned()),
    );
    decision.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("reasoning-search".to_owned()),
    );

    let denied = resolve_dispatch_decision(decision, &payload);

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::SubagentReceiptRequired);
    assert_ne!(
        denied.fields.get("dispatchSatisfied"),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn registered_read_only_subagent_native_edit_is_physically_denied() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "dispatchSatisfied".to_owned(),
        serde_json::Value::Bool(true),
    );
    let actions = crate::tool_action::collect_tool_actions(
        "Edit",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );

    let denied =
        enforce_registered_subagent_capability(decision, &subagent_payload(true), &actions);

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::ReadOnlySubagentWrite);
    assert!(denied.message.contains("asp_explorer"));
    assert!(denied.message.contains("edit action"));
}

#[test]
fn registered_read_only_subagent_shell_edit_capability_is_physically_denied() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "dispatchSatisfied".to_owned(),
        serde_json::Value::Bool(true),
    );
    let action = crate::tool_action::ToolAction::normalized_shell_command_action(
        "opaque > out.txt".to_owned(),
        "Bash".to_owned(),
    );

    let denied =
        enforce_registered_subagent_capability(decision, &subagent_payload(true), &[action]);

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::ReadOnlySubagentWrite);
}

#[test]
fn registered_read_only_denial_dominates_an_existing_policy_denial() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.decision = DecisionKind::Deny;
    decision.reason_kind = ReasonKind::SemanticAstPatchRequired;
    decision.message = "lower-priority policy denial".to_owned();
    let actions = crate::tool_action::collect_tool_actions(
        "Edit",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );

    let denied =
        enforce_registered_subagent_capability(decision, &subagent_payload(true), &actions);

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::ReadOnlySubagentWrite);
    assert_eq!(
        denied.message,
        "编辑模式必须在主线程完成，当前的 asp_explorer 不允许执行 edit action。"
    );
}

#[test]
fn registered_explorer_profile_denies_unscoped_read_capability() {
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "dispatchSatisfied".to_owned(),
        serde_json::Value::Bool(true),
    );
    let action =
        crate::tool_action::ToolAction::normalized_direct_policy_action("src/lib.rs".to_owned());

    let denied =
        enforce_registered_subagent_capability(decision, &subagent_payload(true), &[action]);

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::SubagentCapabilityDenied);
}

#[test]
fn explicit_no_agent_bypass_is_a_complete_subagent_passthrough() {
    let runtime = crate::HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    for event in ["pre-tool", "permission-request", "post-tool"] {
        let mut payload = subagent_payload(false);
        payload["tool_name"] = serde_json::Value::String("Bash".to_owned());
        payload["tool_input"] = serde_json::json!({
            "command": "ASP_NO_AGENT=1 cargo test -p agent-semantic-hook"
        });
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &crate::ClientHookConfig::default(),
            platform: "codex",
            event,
            payload: &payload,
        });

        assert_eq!(
            decision.decision,
            DecisionKind::Allow,
            "{event}: {decision:?}"
        );
        assert_eq!(decision.reason_kind, ReasonKind::None, "{event}");
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            Some("allow-explicit-no-agent-host-bypass"),
            "{event}"
        );
        assert_eq!(
            decision
                .fields
                .get("bypassScope")
                .and_then(serde_json::Value::as_str),
            Some("host-policy"),
            "{event}"
        );
    }
}
