// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::HookClassificationRequest;
use super::classify_hook_with_config;
use super::classify_tool_actions;
use super::collect_payload_tool_actions;
use crate::ClientHookConfig;
use crate::HookRuntime;
use crate::protocol::DecisionKind;
use crate::protocol::ReasonKind;

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        policy_providers: Vec::new(),
    }
}

fn classify(payload: &serde_json::Value) -> crate::HookDecision {
    let config = ClientHookConfig::default();
    let runtime = runtime();
    classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload,
    })
}

#[test]
fn codex_bash_payload_preserves_registered_asp_search_action_for_policy_matching() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "asp search playbook --language rust --rg HookDecision"
        }
    });
    let actions = collect_payload_tool_actions(&payload);
    assert_eq!(actions.len(), 1, "one shell action must be normalized");

    let config = ClientHookConfig::default();
    let runtime = runtime();
    assert!(
        config
            .classify_candidate(&runtime, "codex", "pre-tool", &actions[0])
            .is_some()
    );
    let request = HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &payload,
    };
    assert!(classify_tool_actions(&request, &actions).is_some());
    assert_eq!(
        classify_hook_with_config(request).decision,
        DecisionKind::Deny
    );
}

#[test]
fn codex_renderable_devenv_wrapped_search_is_denied_before_execution() {
    let decision = classify(&serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "/workspace/.devenv/devenv-profile-exec rtk run 'rg -n 4 .devenv/devenv-profile-exec'"
        }
    }));

    assert_eq!(decision.decision, DecisionKind::Deny, "{decision:#?}");
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(serde_json::Value::as_str),
        Some("deny-shell-search-before-execution")
    );
}

#[test]
fn config_selected_explorer_role_admits_search_across_codex_surfaces() {
    let payloads = [
        serde_json::json!({
            "agent_role": "asp_explorer",
            "tool_name": "exec_command",
            "tool_input": {"cmd": "asp search playbook --language rust --rg HookDecision"}
        }),
        serde_json::json!({
            "agentRole": "asp-explorer",
            "tool_name": "Bash",
            "tool_input": {"command": "asp search playbook --language rust --rg HookDecision"}
        }),
    ];

    for payload in payloads {
        let decision = classify(&payload);
        assert_eq!(decision.decision, DecisionKind::Allow, "{decision:#?}");
        assert_eq!(decision.reason_kind, ReasonKind::None);
        assert_eq!(
            decision
                .fields
                .get("dispatchAdmission")
                .and_then(serde_json::Value::as_str),
            Some("config-agent-role"),
            "payload={payload:#} decision={decision:#?}"
        );
    }
}

#[test]
fn config_selected_explorer_role_admits_exact_structured_projection() {
    let payload = serde_json::json!({
        "agent_role": "asp_explorer",
        "tool_name": "Bash",
        "tool_input": {
            "command": "asp query playbook --language rust --selector rust://crates/example.rs#item/function/example --projection source"
        }
    });
    let decision = classify(&payload);
    assert_eq!(decision.decision, DecisionKind::Allow, "{decision:#?}");
    assert_eq!(
        decision
            .fields
            .get("intent")
            .and_then(serde_json::Value::as_str),
        Some("structured-projection")
    );
}

#[test]
fn temporary_subagent_topology_cannot_satisfy_configured_agent_dispatch() {
    let payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "temporary-child",
        "agent_type": "explorer",
        "tool_name": "Bash",
        "tool_input": {"command": "asp search playbook --language rust --rg owner"}
    });
    let decision = classify(&payload);
    assert_eq!(decision.decision, DecisionKind::Deny, "{decision:#?}");
    assert_eq!(decision.reason_kind, ReasonKind::AgentChoiceRequired);
}

#[test]
fn legacy_self_reported_registration_fields_have_no_authority() {
    let payload = serde_json::json!({
        "is_subagent": true,
        "registration_verified": true,
        "registered_agent_name": "asp_explorer",
        "registered_allowed_rule_intents": ["reasoning-search"],
        "tool_name": "Bash",
        "tool_input": {"command": "asp search playbook --language rust --rg owner"}
    });
    let decision = classify(&payload);
    assert_eq!(decision.decision, DecisionKind::Deny, "{decision:#?}");
    assert_eq!(decision.reason_kind, ReasonKind::AgentChoiceRequired);
    assert!(decision.fields.get("dispatchAdmission").is_none());
}

#[test]
fn wrong_config_agent_role_fails_closed_without_cross_route_admission() {
    let payload = serde_json::json!({
        "agent_role": "asp_testing",
        "tool_name": "Bash",
        "tool_input": {"command": "asp search playbook --language rust --rg owner"}
    });
    let decision = classify(&payload);
    assert_eq!(decision.decision, DecisionKind::Deny, "{decision:#?}");
    assert_eq!(decision.reason_kind, ReasonKind::AgentChoiceRequired);
}
