pub(super) use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLoopState, AgentSessionRecord, ResidentChildBootstrapMenuInput,
    SameChildRuntimeOverrideState, agent_session_message_target_is_currently_routable,
    agent_session_message_target_is_live_bound, classify_same_child_runtime_override_state,
    resident_child_bootstrap_menu, resident_child_host_runtime_refresh_eligible,
    resident_child_runtime_repair_menu, typed_runtime_observation_matches_profile,
};

pub(super) fn active_record(
    model: Option<&str>,
    message_target_id: Option<&str>,
) -> AgentSessionRecord {
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

pub(super) fn rollout_and_host_tree_bound_record() -> AgentSessionRecord {
    let mut record = active_record(Some("gpt-5.4-mini"), Some("/root/asp_explorer"));
    record.metadata_json = serde_json::json!({
        "messageTargetBinding": {
            "source": "codex-rollout-session-meta-plus-native-host-tree",
            "boundRootSessionId": "root",
            "childSessionId": "child",
            "messageTargetId": "/root/asp_explorer",
            "observedAt": 2,
        }
    })
    .to_string();
    record
}

pub(super) fn testing_record(message_target_id: Option<&str>) -> AgentSessionRecord {
    let mut record = active_record(Some("gpt-5.4-mini"), message_target_id);
    record.name = "asp-testing".to_string();
    record.role = "build,subagent,testing".to_string();
    record.configured_agent_type = Some("asp_testing".to_string());
    record
}

#[test]
pub(super) fn testing_typed_runtime_is_validated_against_its_own_profile() {
    assert!(typed_runtime_observation_matches_profile(
        "asp_testing",
        "asp_testing",
        "gpt-5.4-mini",
        Some("low"),
        "subagent-start",
        Some("gpt-5.4-mini"),
        Some("low"),
    ));
    assert!(!typed_runtime_observation_matches_profile(
        "asp_explorer",
        "asp_testing",
        "gpt-5.4-mini",
        Some("low"),
        "subagent-start",
        Some("gpt-5.4-mini"),
        Some("low"),
    ));
}
