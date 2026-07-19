use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    SameChildRuntimeOverrideState, agent_session_message_target_is_currently_routable,
    agent_session_message_target_is_live_bound, classify_same_child_runtime_override_state,
    resident_child_bootstrap_menu, resident_child_host_runtime_refresh_eligible,
    resident_child_runtime_repair_menu, typed_runtime_observation_matches_profile,
};

use super::common::{active_record, rollout_and_host_tree_bound_record, testing_record};

#[test]
fn aligned_routable_record_is_ready() {
    let record = active_record(Some("gpt-5.4-mini"), Some("target"));
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

    assert_eq!(menu.state, AgentSessionLoopState::Ready);
    assert_eq!(menu.choices.len(), 1);
    assert_eq!(menu.choices[0].id, "dispatch-resident-command");
    assert_eq!(
        menu.choices[0].next_state,
        AgentSessionLoopState::WaitReceipt
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("only action=send may deliver once")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("must never resend")
    );
    assert_eq!(
        menu.choices[0].required_inputs,
        &["residentCommand", "dispatchIdentity"]
    );
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Classify,
            AgentSessionLoopState::Validate,
            AgentSessionLoopState::Ready,
        ]
    );
    assert_eq!(menu.trace[2].result, "resident-child-ready");
}

#[test]
fn host_tree_observation_prevents_persisted_target_false_ready() {
    let record = active_record(Some("gpt-5.4-mini"), Some("child"));
    let ready = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
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
    assert_eq!(ready.state, AgentSessionLoopState::Ready);

    let blocked =
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            ready,
            "absent",
            Some("absent"),
        );
    assert_eq!(blocked.state, AgentSessionLoopState::Blocked);
    assert_eq!(blocked.choices.len(), 1);
    assert_eq!(
        blocked.choices[0].id,
        "report-host-typed-spawn-capability-unavailable"
    );
    assert!(
        blocked.choices[0]
            .platform_action
            .contains("Unrelated Codex tools remain available")
    );
    assert_eq!(
        blocked.trace.last().map(|step| step.result),
        Some("canonical-host-target-absent-registry-orphan-risk")
    );
}

#[test]
fn host_tree_absent_with_typed_spawn_allows_one_canonical_replacement() {
    let record = active_record(Some("gpt-5.4-mini"), Some("child"));
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
    let repair =
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            menu,
            "absent",
            Some("present"),
        );

    assert_eq!(repair.state, AgentSessionLoopState::Audit);
    assert_eq!(repair.choices.len(), 1);
    assert_eq!(
        repair.choices[0].id,
        "create-canonical-typed-child-after-orphaned-owner"
    );
    assert!(
        repair.choices[0]
            .platform_action
            .contains("task_name=asp_explorer")
    );
}

#[test]
fn historical_orphan_is_never_offered_as_a_live_rebind_target() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("historical-child"));
    record.status = "orphan-risk".to_string();
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("locked-existing-repair-candidate"),
        rollout_history_action: Some("preserve-candidate-identity-until-host-classification"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::Cleanup);
    assert!(
        menu.choices
            .iter()
            .all(|choice| choice.id != "resume-existing-child-for-live-target-rebind")
    );
    assert_eq!(
        menu.trace.last().map(|step| step.result),
        Some("historical-or-stale-child-not-live-rebindable")
    );
}

#[test]
fn host_present_completed_resident_is_resumed_instead_of_cleaned_up() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("child"));
    record.status = "archived".to_string();
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("locked-existing-repair-candidate"),
        rollout_history_action: Some("preserve-candidate-identity-until-host-classification"),
        now: 2,
    });
    assert_eq!(menu.state, AgentSessionLoopState::Cleanup);

    let repair =
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            menu,
            "present",
            Some("present"),
        );

    assert_eq!(
        repair.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert_eq!(repair.choices.len(), 1);
    assert_eq!(
        repair.choices[0].id,
        "resume-existing-child-for-live-target-rebind"
    );
    assert!(
        repair.choices[0]
            .platform_action
            .contains("remain resumable")
    );
    assert!(
        repair
            .choices
            .iter()
            .all(|choice| choice.id != "close-stale-resident-child")
    );
    assert_eq!(
        repair.trace.last().map(|step| step.result),
        Some("canonical-host-target-present-completed-resumable")
    );
}

#[test]
fn historical_unbound_candidate_requires_host_tree_audit_before_resume() {
    let record = active_record(Some("gpt-5.4-mini"), None);
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("locked-existing-repair-candidate"),
        rollout_history_action: Some("preserve-candidate-identity-until-host-classification"),
        now: 2,
    });
    assert_eq!(menu.state, AgentSessionLoopState::RebindExistingChildTarget);

    let audit = agent_semantic_client_db::agent_session_registry::resident_child_host_tree_audit_required_menu(menu);
    assert_eq!(audit.state, AgentSessionLoopState::Audit);
    assert_eq!(audit.choices.len(), 1);
    assert_eq!(
        audit.choices[0].id,
        "audit-host-agent-tree-before-live-target-rebind"
    );
    assert!(
        audit
            .choices
            .iter()
            .all(|choice| choice.id != "resume-existing-child-for-live-target-rebind")
    );
}

#[test]
fn typed_runtime_match_requires_observed_low_when_profile_expects_low() {
    assert!(
        agent_semantic_client_db::agent_session_registry::typed_runtime_observation_matches_profile(
            "asp_explorer",
            "asp_explorer",
            "gpt-5.4-mini",
            Some("low"),
            "subagent-start",
            Some("gpt-5.4-mini"),
            Some("low"),
        )
    );
    assert!(
        agent_semantic_client_db::agent_session_registry::typed_runtime_observation_matches_profile(
            "asp_explorer",
            "asp_explorer",
            "gpt-5.4-mini",
            Some("low"),
            "codex-app-server-thread-resume-after-subagent-start",
            Some("gpt-5.4-mini"),
            Some("low"),
        )
    );
    assert!(
        !agent_semantic_client_db::agent_session_registry::typed_runtime_observation_matches_profile(
            "asp_explorer",
            "asp_explorer",
            "gpt-5.4-mini",
            None,
            "subagent-start",
            Some("gpt-5.4-mini"),
            Some("low"),
        )
    );
}

#[test]
fn incomplete_typed_runtime_evidence_preserves_child_and_forbids_cleanup() {
    let record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("typed-replacement-observed"),
        rollout_history_action: Some("validate-runtime-before-ready"),
        now: 2,
    });

    let menu = agent_semantic_client_db::agent_session_registry::resident_child_runtime_evidence_incomplete_menu(menu);
    assert_eq!(menu.state, AgentSessionLoopState::Blocked);
    assert_eq!(menu.choices.len(), 1);
    assert_eq!(
        menu.choices[0].id,
        "report-host-runtime-reasoning-evidence-unavailable"
    );
    assert!(
        menu.choices
            .iter()
            .all(|choice| choice.id != "close-stale-resident-child")
    );
}

#[test]
fn serialized_menu_is_choice_only_and_keeps_message_target() {
    let record = active_record(Some("gpt-5.4-mini"), Some("target"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "native-host",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });

    let value = serde_json::to_value(&menu).expect("serialize interactive menu");
    assert!(value.get("nextCommand").is_none());
    assert_eq!(value["receipt"]["noNextCommand"], true);
    assert_eq!(value["hostRequirement"]["platform"], "native-host");
    assert_eq!(value["expectedReasoningEffort"], "low");
    assert_eq!(value["session"]["messageTargetId"], "target");
    assert_eq!(value["choices"][0]["id"], "dispatch-resident-command");
}
