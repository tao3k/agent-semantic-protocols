use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, ResidentChildBootstrapMenuInput,
    agent_session_message_target_is_live_bound, resident_child_bootstrap_menu,
    resident_child_host_runtime_refresh_eligible,
};

use super::common::active_record;

#[test]
fn missing_record_requires_audit_before_create() {
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: None,
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: None,
        rollout_history_action: None,
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::Audit);
    assert_eq!(menu.choices.len(), 1);
    assert_eq!(menu.choices[0].id, "audit-resident-candidates");
    assert_eq!(menu.choices[0].next_state, AgentSessionLoopState::Classify);
    assert_eq!(menu.host_requirement.platform, "codex");
    assert_eq!(menu.host_requirement.resident_child_name, "asp-explore");
    assert_eq!(menu.host_requirement.managed_agent_kind, "asp_explorer");
    assert_eq!(
        menu.host_requirement.required_outputs,
        &["childSessionId", "agentMessageTargetId"]
    );
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![AgentSessionLoopState::Audit]
    );
    assert_eq!(menu.trace[0].result, "resident-preflight-required");
    assert!(menu.receipt.no_next_command);
}

#[test]
fn checked_rollout_miss_offers_managed_create_or_host_blocker() {
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: None,
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("checked-no-reusable-rollout"),
        rollout_history_action: Some("create-resident-child-after-rollout-history-miss"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::Audit);
    assert_eq!(menu.choices.len(), 5);
    assert_eq!(
        menu.choices[0].id,
        "audit-host-agent-tree-for-existing-resident-child"
    );
    assert_eq!(menu.choices[1].id, "resume-existing-host-resident-child");
    assert_eq!(menu.choices[2].id, "audit-host-typed-spawn-schema");
    assert_eq!(
        menu.choices[2].required_inputs,
        &["hostTypedSpawnObservation"]
    );
    assert!(
        menu.choices[2]
            .platform_action
            .contains("observe-host-capability")
    );
    assert_eq!(
        menu.choices[3].id,
        "create-managed-resident-child-after-host-tree-miss"
    );
    assert!(
        menu.choices[3]
            .platform_action
            .contains("task_name=asp_explorer")
    );
    assert!(
        menu.host_requirement
            .blocked_when
            .contains(&"native-built-in-agent-type-only")
    );
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Audit,
            AgentSessionLoopState::Classify
        ]
    );
    assert_eq!(
        menu.trace[1].result,
        "registry-missing-host-tree-audit-required"
    );
}

#[test]
fn missing_model_requires_native_profile_observation() {
    for model in [None, Some("unknown")] {
        let record = active_record(model, Some("target"));
        let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
            platform: "codex",
            name: "asp-explore",
            root_session_id: Some("root"),
            record: Some(&record),
            expected_model: Some("gpt-5.4-mini"),
            expected_reasoning_effort: Some("low"),
            rollout_history_status: Some("not-needed"),
            rollout_history_action: Some("none"),
            now: 2,
        });

        assert_eq!(menu.state, AgentSessionLoopState::Validate);
        assert_eq!(
            menu.choices[0].id,
            "resume-existing-child-for-runtime-observation"
        );
        assert!(
            menu.choices[0]
                .platform_action
                .contains("Missing observation is not drift")
        );
        assert_eq!(
            menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
            vec![
                AgentSessionLoopState::Classify,
                AgentSessionLoopState::Validate,
            ]
        );
        assert_eq!(menu.trace[1].result, "model-observation-missing");
    }
}

#[test]
fn model_mismatch_requires_validation_choice() {
    let record = active_record(Some("gpt-5.5"), Some("target"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::Repair);
    assert_eq!(
        menu.choices[0].id,
        "retire-drifted-child-and-create-configured-replacement"
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("agent_type=asp_explorer")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("task_name=asp_explorer")
    );
    assert_eq!(menu.choices[0].next_state, AgentSessionLoopState::Audit);
    assert_eq!(menu.expected_model, Some("gpt-5.4-mini"));
    assert_eq!(menu.expected_reasoning_effort, Some("low"));
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Classify,
            AgentSessionLoopState::Repair,
        ]
    );
    assert_eq!(menu.trace[1].result, "model-mismatch");
}

#[test]
fn missing_message_target_requires_same_child_live_rebind() {
    let record = active_record(Some("gpt-5.4-mini"), None);
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::RebindExistingChildTarget);
    assert_eq!(
        menu.choices[0].id,
        "resume-existing-child-for-live-target-rebind"
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("immediately re-enter this pane")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("fresh same-root host-tree target observation")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("do not create a replacement")
    );
    assert_eq!(menu.trace[1].result, "live-collaboration-target-unbound");
}

#[test]
fn stale_persisted_target_is_unbound_and_requires_same_child_rebind() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("target"));
    record.metadata_json = "{}".to_string();

    assert!(!agent_session_message_target_is_live_bound(&record, "root"));
    assert!(!resident_child_host_runtime_refresh_eligible(
        false, &record, "root"
    ));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("existing-child-discovered"),
        rollout_history_action: Some("resume-existing-child-then-bind-target"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::RebindExistingChildTarget);
    assert_eq!(
        menu.session
            .as_ref()
            .map(|session| session.message_target_status),
        Some("unbound")
    );
    assert!(
        menu.choices
            .iter()
            .all(|choice| !choice.id.contains("create"))
    );
}

#[test]
fn wrong_root_binding_is_not_ready() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("target"));
    record.metadata_json = serde_json::json!({
        "messageTargetBinding": {
            "source": "codex.subagent-start",
            "boundRootSessionId": "stale-root",
            "childSessionId": "child",
            "messageTargetId": "target",
            "observedAt": 1,
        }
    })
    .to_string();

    assert!(!agent_session_message_target_is_live_bound(&record, "root"));
}

#[test]
fn model_observation_refresh_preserves_independent_live_binding() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("target"));
    record.model_observation_source = Some("codex.rollout".to_string());
    record.model_observed_at = Some(2);

    assert!(agent_session_message_target_is_live_bound(&record, "root"));
}
