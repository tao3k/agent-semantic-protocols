use agent_semantic_client_db::agent_session_registry::{
    resident_child_bootstrap_menu, AgentSessionLoopState, AgentSessionRecord,
    ResidentChildBootstrapMenuInput,
};

fn active_record(model: Option<&str>, message_target_id: Option<&str>) -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".to_string(),
        root_session_id: "root".to_string(),
        session_id: "child".to_string(),
        message_target_id: message_target_id.map(str::to_string),
        parent_session_id: Some("root".to_string()),
        name: "asp-explore".to_string(),
        role: "subagent,search".to_string(),
        model: model.map(str::to_string),
        model_observation_source: model.map(|_| "codex.subagent-start".to_string()),
        model_observed_at: model.map(|_| 1),
        model_evidence_ref: model.map(|_| "turn:test".to_string()),
        status: "active".to_string(),
        created_at: 1,
        updated_at: 1,
        last_seen_at: Some(1),
        last_heartbeat_at: Some(1),
        expires_at: None,
        archived_at: None,
        last_tool_event: None,
        last_command: None,
        last_evidence_ref: None,
        metadata_json: "{}".to_string(),
    }
}

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

    assert_eq!(menu.state, AgentSessionLoopState::Create);
    assert_eq!(menu.choices.len(), 2);
    assert_eq!(menu.choices[0].id, "create-managed-resident-child");
    assert_eq!(menu.choices[0].next_state, AgentSessionLoopState::Audit);
    assert!(menu.choices[0].required_inputs.is_empty());
    assert!(menu.choices[0]
        .platform_action
        .contains("Immediately re-enter this pane"));
    assert!(menu.choices[0]
        .platform_action
        .contains("do not wait for SubagentStart as a child message"));
    assert!(menu.choices[0]
        .platform_action
        .contains("instead of creating a duplicate"));
    assert!(menu.choices[0]
        .platform_action
        .contains("detected platform-native managed-agent creation surface"));
    assert!(menu.choices[0]
        .platform_action
        .contains("do not create generic fallback agents or normal threads"));
    assert!(menu.choices[0]
        .platform_action
        .contains("Do not copy child ids, message targets, or model claims"));
    assert_eq!(
        menu.choices[1].id,
        "report-host-managed-agent-lifecycle-unavailable"
    );
    assert_eq!(menu.choices[1].next_state, AgentSessionLoopState::Create);
    assert!(menu
        .host_requirement
        .blocked_when
        .contains(&"native-built-in-agent-type-only"));
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Audit,
            AgentSessionLoopState::Classify
        ]
    );
    assert_eq!(menu.trace[1].result, "no-resident-child");
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

        assert_eq!(menu.state, AgentSessionLoopState::Ready);
        assert_eq!(menu.choices[0].id, "send-denied-asp-command");
        assert_eq!(
            menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
            vec![
                AgentSessionLoopState::Classify,
                AgentSessionLoopState::Validate,
                AgentSessionLoopState::Ready,
            ]
        );
        assert_eq!(menu.trace[1].result, "profile-observation-pending");
        assert_eq!(menu.trace[2].result, "resident-child-ready");
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

    assert_eq!(menu.state, AgentSessionLoopState::Ready);
    assert_eq!(menu.choices[0].id, "send-denied-asp-command");
    assert!(menu.choices[0]
        .platform_action
        .contains("host-native message-agent send"));
    assert!(menu.choices[0]
        .platform_action
        .contains("registered agentMessageTargetId"));
    assert_eq!(
        menu.choices[0].next_state,
        AgentSessionLoopState::WaitReceipt
    );
    assert_eq!(menu.expected_model, Some("gpt-5.4-mini"));
    assert_eq!(menu.expected_reasoning_effort, Some("low"));
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Classify,
            AgentSessionLoopState::Validate,
            AgentSessionLoopState::Ready,
        ]
    );
    assert_eq!(menu.trace[1].result, "role-nickname-model-target-pass");
}

#[test]
fn missing_message_target_can_recover_host_single_agent_id() {
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

    assert_eq!(menu.state, AgentSessionLoopState::Recover);
    assert_eq!(menu.choices[0].id, "resume-managed-child-for-native-start");
    assert!(menu.choices[0]
        .platform_action
        .contains("immediately re-enter this pane"));
    assert!(menu.choices[0]
        .platform_action
        .contains("Do not wait for SubagentStart as a child message"));
    assert!(menu.choices[0]
        .platform_action
        .contains("Do not verify or register a target through child text"));
    assert_eq!(menu.trace[1].result, "native-message-target-unverified");
}

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
    assert_eq!(menu.choices.len(), 2);
    assert_eq!(menu.choices[0].id, "send-denied-asp-command");
    assert_eq!(
        menu.choices[0].next_state,
        AgentSessionLoopState::WaitReceipt
    );
    assert_eq!(menu.choices[1].id, "record-native-child-retirement");
    assert_eq!(menu.choices[1].next_state, AgentSessionLoopState::Cleanup);
    assert_eq!(menu.choices[1].required_inputs, &["nativeStopReceipt"]);
    assert!(menu.choices[1]
        .platform_action
        .contains("previous_status=running means shutdown was requested, not completed"));
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
    assert_eq!(value["choices"][0]["id"], "send-denied-asp-command");
}
