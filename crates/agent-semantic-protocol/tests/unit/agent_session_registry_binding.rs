use agent_semantic_client_db::agent_session_registry::AgentSessionRecord;

use super::insert_absent_canonical_target_receipt;

fn record_for_testing_lane() -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".to_string(),
        root_session_id: "root".to_string(),
        session_id: "child-testing".to_string(),
        physical_generation: 1,
        configured_agent_type: Some("asp_testing".to_string()),
        profile_evidence_json: None,
        message_target_id: Some("/root/asp_testing".to_string()),
        parent_session_id: Some("root".to_string()),
        name: "asp-testing".to_string(),
        role: "asp_testing".to_string(),
        model: Some("gpt-5.4-mini".to_string()),
        model_observation_source: Some("test".to_string()),
        model_observed_at: Some(1),
        model_evidence_ref: Some("child-testing".to_string()),
        status: "orphan-risk".to_string(),
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
fn absent_canonical_binding_receipt_uses_lane_target() {
    let record = record_for_testing_lane();
    let mut object = serde_json::Map::new();

    insert_absent_canonical_target_receipt(
        &mut object,
        Some(&record),
        Some("present"),
        "/root/asp_testing",
    );

    let binding = object
        .get("canonicalBindingObservation")
        .and_then(|value| value.as_object())
        .expect("canonical binding observation");

    assert_eq!(
        binding
            .get("canonicalTarget")
            .and_then(|value| value.as_str()),
        Some("/root/asp_testing")
    );
    assert_ne!(
        binding
            .get("canonicalTarget")
            .and_then(|value| value.as_str()),
        Some("/root/asp_explorer")
    );
}
