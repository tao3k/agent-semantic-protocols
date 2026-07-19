use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    SameChildRuntimeOverrideState, agent_session_message_target_is_currently_routable,
    agent_session_message_target_is_live_bound, classify_same_child_runtime_override_state,
    resident_child_bootstrap_menu, resident_child_host_runtime_refresh_eligible,
    resident_child_runtime_repair_menu, typed_runtime_observation_matches_profile,
};

use super::common::{active_record, rollout_and_host_tree_bound_record, testing_record};

#[test]
fn runtime_verified_registry_requires_fresh_live_transport_before_ready() {
    let record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    let make_menu = || {
        resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
            platform: "codex",
            name: "asp-explore",
            root_session_id: Some("root"),
            record: Some(&record),
            expected_model: Some("gpt-5.4-mini"),
            expected_reasoning_effort: Some("low"),
            rollout_history_status: Some("not-needed"),
            rollout_history_action: Some("none"),
            now: 2,
        })
    };

    let rejected =
        agent_semantic_client_db::agent_session_registry::resident_child_runtime_verified_menu(
            make_menu(),
            true,
            false,
        );
    assert_eq!(
        rejected.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert_eq!(
        rejected
            .session
            .as_ref()
            .map(|session| session.message_target_status),
        Some("unbound")
    );
    assert_eq!(
        rejected.choices[0].id,
        "verify-live-resident-transport-before-dispatch"
    );

    let ready =
        agent_semantic_client_db::agent_session_registry::resident_child_runtime_verified_menu(
            make_menu(),
            true,
            true,
        );
    assert_eq!(ready.state, AgentSessionLoopState::Ready);
    assert_eq!(ready.choices[0].id, "send-denied-asp-command");
}

#[test]
fn builder_rollout_miss_create_choice_never_falls_back_to_explorer() {
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-builder",
        root_session_id: Some("root"),
        record: None,
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("checked-no-reusable-rollout"),
        rollout_history_action: Some("create-resident-child-after-rollout-history-miss"),
        now: 2,
    });

    assert_eq!(menu.state, AgentSessionLoopState::Audit);
    assert_eq!(
        menu.choices[3].id,
        "create-managed-resident-child-after-host-tree-miss"
    );
    assert_eq!(menu.host_requirement.resident_child_name, "asp-builder");
    assert_eq!(
        menu.host_requirement.managed_agent_kind.as_ref(),
        "asp_builder"
    );

    let action = menu.choices[3].platform_action.as_ref();
    assert!(action.contains("agent_type=asp_builder"));
    assert!(action.contains("task_name=asp_builder"));
    assert!(action.contains("/root/asp_builder"));
    assert!(!action.contains("agent_type=asp_explorer"));
    assert!(!action.contains("/root/asp_explorer"));
}

#[test]
fn builder_orphan_replacement_never_falls_back_to_explorer() {
    let record = active_record(Some("gpt-5.4-mini"), Some("child"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-builder",
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

    let action = repair.choices[0].platform_action.as_ref();
    assert!(action.contains("agent_type=asp_builder"));
    assert!(action.contains("task_name=asp_builder"));
    assert!(action.contains("/root/asp_builder"));
    assert!(!action.contains("agent_type=asp_explorer"));
    assert!(!action.contains("/root/asp_explorer"));
}
