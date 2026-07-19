use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    agent_session_message_target_is_live_bound, resident_child_bootstrap_menu,
    resident_child_host_tree_observation_menu,
};

fn active_record(model: Option<&str>, message_target_id: Option<&str>) -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".to_string(),
        root_session_id: "root".to_string(),
        session_id: "child".to_string(),
        physical_generation: 1,
        configured_agent_type: Some("asp_explorer".to_string()),
        profile_evidence_json: None,
        message_target_id: message_target_id.map(str::to_string),
        parent_session_id: Some("root".to_string()),
        name: "asp-explore".to_string(),
        role: "asp_explorer".to_string(),
        model: model.map(str::to_string),
        model_observation_source: model.map(|_| "test".to_string()),
        model_observed_at: model.map(|_| 1),
        model_evidence_ref: Some("child".to_string()),
        status: "idle".to_string(),
        created_at: 1,
        updated_at: 1,
        last_seen_at: Some(1),
        last_heartbeat_at: Some(1),
        expires_at: None,
        archived_at: None,
        last_tool_event: None,
        last_command: None,
        last_evidence_ref: None,
        metadata_json: serde_json::json!({
            "messageTargetBinding": {
                "source": "codex.subagent-start",
                "boundRootSessionId": "root",
                "childSessionId": "child",
                "messageTargetId": message_target_id.unwrap_or("/root/asp_explorer"),
                "observedAt": 1,
            }
        })
        .to_string(),
    }
}

#[test]
fn stale_binding_followup_ack_restores_ready_without_create() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    record.metadata_json = "{}".to_string();

    let unbound = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
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

    assert_eq!(
        unbound.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert!(
        unbound
            .choices
            .iter()
            .all(|choice| !choice.id.contains("create"))
    );

    record.metadata_json = serde_json::json!({
        "messageTargetBinding": {
            "source": "native-collaboration-followup-ack",
            "boundRootSessionId": "root",
            "childSessionId": "child",
            "messageTargetId": "/root/asp_explorer",
            "observedAt": 3,
        }
    })
    .to_string();

    assert!(agent_session_message_target_is_live_bound(&record, "root"));

    let ready = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 3,
    });

    assert_eq!(ready.state, AgentSessionLoopState::Ready);
    assert_eq!(ready.choices.len(), 1);
    assert_eq!(ready.choices[0].id, "dispatch-resident-command");
    assert!(
        ready
            .choices
            .iter()
            .all(|choice| !choice.id.contains("create")
                && !choice.id.contains("replace")
                && !choice.id.contains("close"))
    );
}

#[test]
fn host_present_running_resident_is_resumed_not_duplicated() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("child"));
    record.metadata_json = "{}".to_string();
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

    let repair = resident_child_host_tree_observation_menu(menu, "present", Some("present"));

    assert_eq!(
        repair.state,
        AgentSessionLoopState::RebindExistingChildTarget
    );
    assert_eq!(
        repair.choices[0].id,
        "resume-existing-child-for-live-target-rebind"
    );
    assert!(
        repair.choices[0]
            .platform_action
            .contains("observe-host-ack --name asp-explore --canonical-target /root/asp_explorer"),
        "{}",
        repair.choices[0].platform_action
    );
    assert!(
        repair
            .choices
            .iter()
            .all(|choice| !choice.id.contains("create"))
    );
    assert!(
        repair
            .choices
            .iter()
            .all(|choice| !choice.id.contains("duplicate"))
    );
}

#[test]
fn host_tree_absent_requires_canonical_probe_before_replacement() {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    record.metadata_json = "{}".to_string();
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

    let probe = resident_child_host_tree_observation_menu(menu, "absent", Some("present"));

    assert_eq!(probe.state, AgentSessionLoopState::Audit);
    assert_eq!(
        probe.choices[0].id,
        "probe-hidden-routable-child-before-replacement"
    );
    assert!(
        probe
            .choices
            .iter()
            .all(|choice| !choice.id.contains("create"))
    );
    assert!(probe.choices.iter().all(|choice| {
        !matches!(
            choice.id,
            "create-canonical-typed-child-after-orphaned-owner"
                | "retire-drifted-child-and-create-configured-replacement"
                | "close-stale-resident-child"
                | "dispatch-resident-command"
        )
    }));
}

#[test]
fn no_child_create_then_bound_record_reaches_ready() {
    let create = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
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

    assert_eq!(create.state, AgentSessionLoopState::Audit);
    assert_eq!(
        create.choices[3].id,
        "create-managed-resident-child-after-host-tree-miss"
    );

    let record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    let ready = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(&record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: Some("not-needed"),
        rollout_history_action: Some("none"),
        now: 3,
    });

    assert_eq!(ready.state, AgentSessionLoopState::Ready);
    assert_eq!(ready.choices[0].id, "dispatch-resident-command");
}
