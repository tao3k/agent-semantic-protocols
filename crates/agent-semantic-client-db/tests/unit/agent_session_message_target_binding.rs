use super::{AgentSessionRecord, agent_session_message_target_is_live_bound};

fn record_with_binding_source(source: &str) -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".into(),
        root_session_id: "root".into(),
        session_id: "child".into(),
        physical_generation: 1,
        configured_agent_type: Some("asp_explorer".to_string()),
        profile_evidence_json: None,
        message_target_id: Some("/root/asp_explorer".to_string()),
        parent_session_id: Some("root".into()),
        name: "asp-explore".into(),
        role: "asp_explorer".to_string(),
        model: Some("gpt-5.4-mini".to_string()),
        model_observation_source: Some("codex.rollout".to_string()),
        model_observed_at: Some(1),
        model_evidence_ref: Some("child".to_string()),
        status: "idle".into(),
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
                "source": source,
                "boundRootSessionId": "root",
                "childSessionId": "child",
                "messageTargetId": "/root/asp_explorer",
                "observedAt": 1,
            }
        })
        .to_string(),
    }
}

#[test]
fn locked_generation_profile_plus_native_host_tree_is_live_bound() {
    let record =
        record_with_binding_source("codex-locked-generation-profile-plus-native-host-tree");

    assert!(agent_session_message_target_is_live_bound(&record, "root"));
}

#[test]
fn unknown_binding_source_is_not_live_bound() {
    let record = record_with_binding_source("untrusted-host-observation");

    assert!(!agent_session_message_target_is_live_bound(&record, "root"));
}
