use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    SameChildRuntimeOverrideState, agent_session_message_target_is_currently_routable,
    agent_session_message_target_is_live_bound, classify_same_child_runtime_override_state,
    resident_child_bootstrap_menu, resident_child_host_runtime_refresh_eligible,
    resident_child_runtime_repair_menu, typed_runtime_observation_matches_profile,
};

use super::common::{active_record, rollout_and_host_tree_bound_record, testing_record};

#[test]
fn testing_resident_projects_testing_profile_and_canonical_target() {
    let record = testing_record(Some("/root/asp_testing"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-testing",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });

    assert_eq!(menu.host_requirement.resident_child_name, "asp-testing");
    assert_eq!(menu.host_requirement.managed_agent_kind, "asp_testing");
    assert_eq!(menu.state, AgentSessionLoopState::Ready);
    assert!(
        menu.choices[0]
            .platform_action
            .contains("configured resident slot")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("verified canonical message target")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("configured receipt kind")
    );
    assert!(menu.choices[0].platform_action.contains("dispatch-claim"));
    assert!(!menu.choices[0].platform_action.contains("asp_explorer"));
}

#[test]
fn testing_host_tree_rebind_never_points_at_explorer() {
    let record = testing_record(None);
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-testing",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("existing-child-discovered"),
        rollout_history_action: Some("resume-existing-child-then-bind-target"),
        now: 2,
    });
    let observed =
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            menu,
            "present",
            Some("present"),
        );

    assert_eq!(
        observed.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert!(
        observed.choices[0]
            .platform_action
            .contains("/root/asp_testing")
    );
    assert!(
        observed.choices[0]
            .platform_action
            .contains("--name asp-testing")
    );
    assert!(!observed.choices[0].platform_action.contains("asp_explorer"));
    assert!(!observed.choices[0].platform_action.contains("asp-explore"));
}

#[test]
fn testing_live_transport_gate_uses_testing_target_and_receipt() {
    let record = testing_record(Some("/root/asp_testing"));
    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-testing",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });
    let rebind =
        agent_semantic_client_db::agent_session_registry::resident_child_runtime_verified_menu(
            menu, true, false,
        );

    assert_eq!(
        rebind.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert!(
        rebind.choices[0]
            .platform_action
            .contains("/root/asp_testing")
    );
    assert!(!rebind.choices[0].platform_action.contains("asp_explorer"));

    let menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-testing",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 2,
    });
    let ready =
        agent_semantic_client_db::agent_session_registry::resident_child_runtime_verified_menu(
            menu, true, true,
        );
    assert_eq!(ready.state, AgentSessionLoopState::Ready);
    assert!(
        ready.choices[0]
            .platform_action
            .contains("configured receipt kind")
    );
}

#[test]
fn rollout_identity_plus_native_host_tree_is_a_live_canonical_binding() {
    let record = rollout_and_host_tree_bound_record();

    assert!(
        agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            &record, "root"
        )
    );
}

#[test]
fn durable_identity_binding_requires_fresh_host_transport_for_routing() {
    let record = rollout_and_host_tree_bound_record();

    assert!(agent_session_message_target_is_currently_routable(
        &record, "root", true, 2,
    ));
    assert!(!agent_session_message_target_is_currently_routable(
        &record, "root", false, 2,
    ));
}

#[test]
fn typed_subagent_start_plus_native_host_tree_is_a_live_canonical_binding() {
    let mut record = rollout_and_host_tree_bound_record();
    let mut metadata: serde_json::Value =
        serde_json::from_str(&record.metadata_json).expect("metadata");
    metadata["messageTargetBinding"]["source"] =
        serde_json::Value::String("codex-typed-subagent-start-plus-native-host-tree".to_string());
    record.metadata_json = metadata.to_string();

    assert!(
        agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            &record, "root"
        )
    );
}

#[test]
fn followup_ack_is_a_live_canonical_binding() {
    let mut record = rollout_and_host_tree_bound_record();
    let mut metadata: serde_json::Value =
        serde_json::from_str(&record.metadata_json).expect("metadata");
    metadata["messageTargetBinding"]["source"] =
        serde_json::Value::String("native-collaboration-followup-ack".to_string());
    record.metadata_json = metadata.to_string();

    assert!(
        agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            &record, "root"
        )
    );
}

#[test]
fn rollout_host_tree_binding_rejects_root_child_and_target_mismatch() {
    let record = rollout_and_host_tree_bound_record();

    for (field, value) in [
        ("boundRootSessionId", "other-root"),
        ("childSessionId", "other-child"),
        ("messageTargetId", "/root/other"),
    ] {
        let mut invalid = record.clone();
        let mut metadata: serde_json::Value =
            serde_json::from_str(&invalid.metadata_json).expect("metadata");
        metadata["messageTargetBinding"][field] = serde_json::Value::String(value.to_string());
        invalid.metadata_json = metadata.to_string();
        assert!(
            !agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
                &invalid, "root"
            ),
            "{field} mismatch must invalidate the binding"
        );
    }
}

#[test]
fn canonical_path_without_trusted_identity_source_is_not_live_bound() {
    let mut record = rollout_and_host_tree_bound_record();
    let mut metadata: serde_json::Value =
        serde_json::from_str(&record.metadata_json).expect("metadata");
    metadata["messageTargetBinding"]["source"] = serde_json::Value::String("path-only".to_string());
    record.metadata_json = metadata.to_string();

    assert!(
        !agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            &record, "root"
        )
    );
}

#[test]
fn pending_fresh_same_child_runtime_mismatch_requires_typed_replacement() {
    let mut record = active_record(Some("gpt-5.6-sol"), Some("/root/asp_explorer"));
    record.status = "replacement-required".to_string();

    assert!(!resident_child_host_runtime_refresh_eligible(
        false, &record, "root"
    ));
    assert_eq!(
        classify_same_child_runtime_override_state(&record.status, false, true),
        SameChildRuntimeOverrideState::ReplacementRequired,
    );

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
    let repair = resident_child_runtime_repair_menu(menu, 2);

    assert_eq!(repair.state, AgentSessionLoopState::Repair);
    assert_eq!(
        repair.trace.last().map(|step| step.result),
        Some("typed-resident-replacement-required")
    );
    assert_eq!(
        repair.choices.first().map(|choice| choice.id),
        Some("retire-drifted-child-and-create-configured-replacement")
    );
}
