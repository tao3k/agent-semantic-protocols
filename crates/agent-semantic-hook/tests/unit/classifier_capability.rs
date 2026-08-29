use super::{
    HookClassificationRequest, classify_hook_with_config, classify_tool_actions,
    collect_payload_tool_actions, enforce_registered_subagent_capability,
    resolve_dispatch_decision,
};
use crate::classifier::decision::allow;
use crate::protocol::{DecisionKind, DecisionSubject, ReasonKind};
use crate::{ClientHookConfig, HookRuntime};

fn subagent_payload(registration_verified: bool) -> serde_json::Value {
    serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "explorer",
        "registration_verified": registration_verified,
        "registered_agent_name": "asp_explorer",
        "registered_denied_actions": ["edit"],
        "registered_allowed_rule_intents": ["reasoning-search"],
    })
}

#[test]
fn codex_bash_payload_preserves_registered_asp_search_action_for_policy_matching() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "asp rust search pipe 'HookDecision' --workspace . --view seeds"
        }
    });
    let actions = collect_payload_tool_actions(&payload);
    assert_eq!(actions.len(), 1, "one shell action must be normalized");

    let config = ClientHookConfig::default();
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    assert!(
        config
            .classify_candidate(&runtime, "codex", "pre-tool", &actions[0])
            .is_some(),
        "the normalized Codex Bash action must admit the registered ASP search rule"
    );
    let request = HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &payload,
    };
    assert!(
        classify_tool_actions(&request, &actions).is_some(),
        "multi-action classification must retain the matching deny candidate"
    );
    assert_eq!(
        classify_hook_with_config(request).decision,
        DecisionKind::Deny,
        "the public classifier must materialize the retained deny candidate"
    );
}

#[test]
fn verified_explorer_profile_admits_registered_search_across_codex_surfaces() {
    let config = ClientHookConfig::default();
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };

    let payloads = [
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "exec_command",
            "tool_input": {
                "cmd": "asp rust search pipe 'HookDecision' --workspace . --view seeds"
            }
        }),
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "Bash",
            "tool_input": {
                "command": "env ASP_VIEW=seeds asp rust search pipe 'HookDecision' --workspace . --view seeds"
            }
        }),
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "Bash",
            "tool_input": {
                "command": "rtk --ultra-compact err asp rust search pipe 'HookDecision' --workspace . --view seeds"
            }
        }),
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "functions.exec",
            "tool_input": {
                "code": "const r = await tools.exec_command({cmd: \"asp rust search pipe 'HookDecision' --workspace . --view seeds\"});"
            }
        }),
    ];

    for payload in &payloads {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload,
        });
        assert_eq!(decision.decision, DecisionKind::Allow, "{decision:#?}");
        assert_eq!(
            decision
                .fields
                .get("dispatchAdmission")
                .and_then(serde_json::Value::as_str),
            Some("verified-registration-intent"),
            "{decision:#?}"
        );
    }

    let post_tool_payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
        "registration_verified": true,
        "registered_agent_name": "asp_explorer",
        "registered_denied_actions": ["edit"],
        "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
        "tool_name": "Bash",
        "tool_input": {
            "command": "asp rust search pipe 'HookDecision' --workspace . --view seeds"
        },
        "tool_response": {
            "exit_code": 2,
            "output": "typed Runtime endpoint failure"
        }
    });
    let post_tool_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "post-tool",
        payload: &post_tool_payload,
    });
    assert_eq!(
        post_tool_decision.decision,
        DecisionKind::Allow,
        "PostToolUse is observational and must not create a new capability denial: {post_tool_decision:#?}"
    );
    assert_eq!(post_tool_decision.reason_kind, ReasonKind::None);
    assert!(
        post_tool_decision
            .fields
            .get("attemptedRuleIntent")
            .is_none(),
        "observational output must not be relabeled as an unscoped command"
    );
}

#[test]
fn verified_explorer_profile_admits_exact_query_as_structured_projection() {
    let config = ClientHookConfig::default();
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let payloads = [
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "exec_command",
            "tool_input": {
                "cmd": "asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source"
            }
        }),
        serde_json::json!({
            "is_subagent": true,
            "agent_id": "child-session",
            "agent_type": "asp_explorer",
            "registration_verified": true,
            "registered_agent_name": "asp_explorer",
            "registered_denied_actions": ["edit"],
            "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
            "tool_name": "Bash",
            "tool_input": {
                "command": "rtk --ultra-compact err asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source"
            }
        }),
    ];

    for payload in &payloads {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload,
        });
        assert_eq!(decision.decision, DecisionKind::Allow, "{decision:#?}");
        assert_eq!(
            decision
                .fields
                .get("intent")
                .and_then(serde_json::Value::as_str),
            Some("structured-projection"),
            "{decision:#?}"
        );
        assert_eq!(
            decision
                .fields
                .get("dispatchAdmission")
                .and_then(serde_json::Value::as_str),
            Some("verified-registration-intent"),
            "{decision:#?}"
        );
    }
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
fn verified_coding_profile_admits_declarative_owner_scoped_edit_without_dispatch_recursion() {
    let mut payload = subagent_payload(true);
    payload["agent_type"] = serde_json::json!("worker");
    payload["registered_agent_name"] = serde_json::json!("asp_coding");
    payload["registered_denied_actions"] = serde_json::json!([]);
    payload["registered_allowed_rule_intents"] = serde_json::json!(["owner-scoped-mutation"]);
    let mut decision = allow("codex", "pre-tool", DecisionSubject::default());
    decision.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("owner-scoped-mutation".to_owned()),
    );
    let actions = crate::tool_action::collect_tool_actions(
        "apply_patch",
        &serde_json::json!({
            "command": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch"
        }),
    );

    let allowed = enforce_registered_subagent_capability(decision, &payload, &actions);

    assert_eq!(allowed.decision, DecisionKind::Allow, "{allowed:#?}");
    assert_eq!(allowed.reason_kind, ReasonKind::None);
    assert_eq!(
        allowed.fields["capabilityAdmission"],
        "verified-registration-intent"
    );
    assert_eq!(allowed.fields["registeredAgentName"], "asp_coding");
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
    assert_eq!(allowed.fields["currentAgentType"], "explorer");
    assert_eq!(allowed.fields["currentRegisteredAgentName"], "asp_explorer");

    let replayed = resolve_dispatch_decision(allowed, &subagent_payload(true));
    assert_eq!(replayed.decision, DecisionKind::Allow);
    assert_eq!(replayed.reason_kind, ReasonKind::None);
    assert_eq!(
        replayed.fields["dispatchAdmission"], "verified-registration-intent",
        "verified dispatch replay must remain idempotently admitted"
    );
}

#[test]
fn native_role_only_matching_is_the_deadlock_witness_and_canonical_rebind_progresses() {
    let payload = subagent_payload(true);
    let native_role = payload["agent_type"].as_str().expect("Host native role");
    let canonical_agent = payload["registered_agent_name"]
        .as_str()
        .expect("verified canonical agent");
    let target_agent = "asp_explorer";

    assert_ne!(
        native_role, target_agent,
        "the Host role is not the configurable ASP identity"
    );
    assert_eq!(canonical_agent, target_agent);

    let mut stale_shard = allow("codex", "pre-tool", DecisionSubject::default());
    stale_shard.decision = DecisionKind::Deny;
    stale_shard.reason_kind = ReasonKind::SubagentReceiptRequired;
    stale_shard.fields.insert(
        "targetAgent".to_owned(),
        serde_json::Value::String(target_agent.to_owned()),
    );
    stale_shard.fields.insert(
        "intent".to_owned(),
        serde_json::Value::String("reasoning-search".to_owned()),
    );

    let progressed = resolve_dispatch_decision(stale_shard, &payload);

    assert_eq!(progressed.decision, DecisionKind::Allow);
    assert_eq!(progressed.reason_kind, ReasonKind::None);
    assert_eq!(
        progressed.fields["dispatchAdmission"],
        "verified-registration-intent"
    );
}

#[test]
fn unverified_canonical_agent_name_cannot_satisfy_dispatch() {
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

    let denied = resolve_dispatch_decision(decision, &subagent_payload(false));

    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(denied.reason_kind, ReasonKind::SubagentReceiptRequired);
}

#[test]
fn wrong_registered_canonical_agent_cannot_satisfy_dispatch() {
    let mut payload = subagent_payload(true);
    payload["registered_agent_name"] = serde_json::json!("asp_testing");
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
    let mut actions = crate::tool_action::collect_tool_actions(
        "Edit",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );

    actions[0].host_action = crate::action_ir::HostInvocationKind::Edit;
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
    let mut actions = crate::tool_action::collect_tool_actions(
        "Edit",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );

    actions[0].host_action = crate::action_ir::HostInvocationKind::Edit;
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
fn process_bound_no_agent_marker_is_subagent_recovery_authority_only_for_pre_tool() {
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

        if event == "pre-tool" {
            assert_eq!(
                decision
                    .fields
                    .get("bypassScope")
                    .and_then(serde_json::Value::as_str),
                Some("host-policy")
            );
        } else {
            assert!(decision.fields.get("bypassScope").is_none(), "{event}");
        }
    }
}
