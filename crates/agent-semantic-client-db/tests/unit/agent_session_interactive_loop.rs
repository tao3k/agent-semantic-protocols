use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    SameChildRuntimeOverrideState, agent_session_message_target_is_live_bound,
    classify_same_child_runtime_override_state, resident_child_bootstrap_menu,
    resident_child_host_runtime_refresh_eligible, resident_child_runtime_repair_menu,
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
        metadata_json: message_target_id.map_or_else(
            || "{}".to_string(),
            |target| {
                serde_json::json!({
                    "messageTargetBinding": {
                        "source": "codex.subagent-start",
                        "boundRootSessionId": "root",
                        "childSessionId": "child",
                        "messageTargetId": target,
                        "observedAt": 1,
                    }
                })
                .to_string()
            },
        ),
    }
}

#[test]
fn pending_fresh_same_child_runtime_mismatch_requires_typed_replacement() {
    let mut record = active_record(Some("gpt-5.6-sol"), Some("child"));
    record.status = "replacement-required".to_string();

    assert!(resident_child_host_runtime_refresh_eligible(
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
    assert_eq!(menu.choices.len(), 6);
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
    assert_eq!(menu.choices[3].id, "activate-inline-parser-fallback");
    assert_eq!(
        menu.choices[4].id,
        "create-managed-resident-child-after-host-tree-miss"
    );
    assert!(
        menu.choices[4]
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
            .contains("fresh same-root SubagentStart lifecycle hook")
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
    assert_eq!(menu.choices[0].id, "send-denied-asp-command");
    assert_eq!(
        menu.choices[0].next_state,
        AgentSessionLoopState::WaitReceipt
    );
    assert!(
        menu.choices[0]
            .platform_action
            .contains("Do not retire it merely because one search turn completed")
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

    let degraded =
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            ready,
            "absent",
            Some("absent"),
        );
    assert_eq!(degraded.state, AgentSessionLoopState::Audit);
    assert_eq!(degraded.choices.len(), 1);
    assert_eq!(degraded.choices[0].id, "activate-inline-parser-fallback");
    assert_eq!(
        degraded.trace.last().map(|step| step.result),
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
