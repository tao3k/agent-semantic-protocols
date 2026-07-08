use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, resident_child_bootstrap_menu,
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
    let menu = resident_child_bootstrap_menu(
        "codex",
        "asp-explore",
        Some("root"),
        None,
        Some("gpt-5.4-mini"),
        None,
        None,
        2,
    );

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
    let menu = resident_child_bootstrap_menu(
        "codex",
        "asp-explore",
        Some("root"),
        None,
        Some("gpt-5.4-mini"),
        Some("checked-no-reusable-rollout"),
        Some("create-resident-child-after-rollout-history-miss"),
        2,
    );

    assert_eq!(menu.state, AgentSessionLoopState::Create);
    assert_eq!(menu.choices.len(), 2);
    assert_eq!(menu.choices[0].id, "create-managed-resident-child");
    assert_eq!(menu.choices[0].next_state, AgentSessionLoopState::Register);
    assert_eq!(
        menu.choices[0].required_inputs,
        &["configuredModel", "childSessionId", "agentMessageTargetId"]
    );
    assert_eq!(
        menu.choices[1].id,
        "report-host-managed-agent-target-unavailable"
    );
    assert_eq!(menu.choices[1].next_state, AgentSessionLoopState::Create);
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
    assert_eq!(menu.trace[1].result, "no-resident-child");
}

#[test]
fn model_mismatch_requires_validation_choice() {
    let record = active_record(Some("gpt-5.5"), Some("target"));
    let menu = resident_child_bootstrap_menu(
        "codex",
        "asp-explore",
        Some("root"),
        Some(&record),
        Some("gpt-5.4-mini"),
        Some("not-needed"),
        Some("none"),
        2,
    );

    assert_eq!(menu.state, AgentSessionLoopState::Validate);
    assert_eq!(menu.choices[0].id, "confirm-configured-model");
    assert_eq!(menu.choices[0].next_state, AgentSessionLoopState::Validate);
    assert_eq!(menu.expected_model, Some("gpt-5.4-mini"));
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Classify,
            AgentSessionLoopState::Validate
        ]
    );
    assert_eq!(menu.trace[1].result, "model-missing-or-mismatch");
}

#[test]
fn missing_message_target_can_recover_host_single_agent_id() {
    let record = active_record(Some("gpt-5.4-mini"), None);
    let menu = resident_child_bootstrap_menu(
        "codex",
        "asp-explore",
        Some("root"),
        Some(&record),
        Some("gpt-5.4-mini"),
        Some("not-needed"),
        Some("none"),
        2,
    );

    assert_eq!(menu.state, AgentSessionLoopState::Recover);
    assert_eq!(menu.choices[0].id, "recover-native-message-target");
    assert!(
        menu.choices[0]
            .platform_action
            .contains("host exposes one single agent id")
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("native send accepts it")
    );
    assert_eq!(menu.trace[1].result, "native-message-target-missing");
}

#[test]
fn aligned_routable_record_is_ready() {
    let record = active_record(Some("gpt-5.4-mini"), Some("target"));
    let menu = resident_child_bootstrap_menu(
        "codex",
        "asp-explore",
        Some("root"),
        Some(&record),
        Some("gpt-5.4-mini"),
        Some("not-needed"),
        Some("none"),
        2,
    );

    assert_eq!(menu.state, AgentSessionLoopState::Ready);
    assert_eq!(menu.choices.len(), 1);
    assert_eq!(menu.choices[0].id, "send-denied-asp-command");
    assert_eq!(
        menu.choices[0].next_state,
        AgentSessionLoopState::WaitReceipt
    );
    assert_eq!(
        menu.trace.iter().map(|step| step.state).collect::<Vec<_>>(),
        vec![
            AgentSessionLoopState::Classify,
            AgentSessionLoopState::Validate,
            AgentSessionLoopState::Ready
        ]
    );
    assert_eq!(menu.trace[2].result, "resident-child-ready");
}

#[test]
fn serialized_menu_is_choice_only_and_keeps_message_target() {
    let record = active_record(Some("gpt-5.4-mini"), Some("target"));
    let menu = resident_child_bootstrap_menu(
        "native-host",
        "asp-explore",
        Some("root"),
        Some(&record),
        Some("gpt-5.4-mini"),
        Some("not-needed"),
        Some("none"),
        2,
    );

    let value = serde_json::to_value(&menu).expect("serialize interactive menu");
    assert!(value.get("nextCommand").is_none());
    assert_eq!(value["receipt"]["noNextCommand"], true);
    assert_eq!(value["hostRequirement"]["platform"], "native-host");
    assert_eq!(value["session"]["messageTargetId"], "target");
    assert_eq!(value["choices"][0]["id"], "send-denied-asp-command");
}
