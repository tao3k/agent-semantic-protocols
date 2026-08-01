use agent_semantic_client_db::agent_session_registry::AgentSessionRecord;

use super::insert_non_present_canonical_target_receipt;

fn record_for_testing_lane() -> AgentSessionRecord {
    AgentSessionRecord {
        project_id: "project".to_string().into(),
        root_session_id: "root".to_string().into(),
        session_id: "child-testing".to_string().into(),
        physical_generation: 1,
        configured_agent_type: Some("asp_testing".to_string().into()),
        profile_evidence_json: None,
        message_target_id: Some("/root/asp_testing".to_string().into()),
        parent_session_id: Some("root".to_string().into()),
        name: "asp-testing".to_string().into(),
        role: "asp_testing".to_string().into(),
        model: Some("gpt-5.4-mini".to_string().into()),
        model_observation_source: Some("test".to_string().into()),
        model_observed_at: Some(1),
        model_evidence_ref: Some("child-testing".to_string().into()),
        status: "orphan-risk".to_string().into(),
        created_at: 1,
        updated_at: 1,
        last_seen_at: Some(1),
        last_heartbeat_at: Some(1),
        expires_at: None,
        archived_at: None,
        last_tool_event: None,
        last_command: None,
        last_evidence_ref: None,
        metadata_json: "{}".to_string().into(),
    }
}

#[test]
fn absent_canonical_binding_receipt_requires_probe_and_uses_lane_target() {
    let record = record_for_testing_lane();
    let mut object = serde_json::Map::new();

    insert_non_present_canonical_target_receipt(
        &mut object,
        Some(&record),
        "absent",
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
    assert_eq!(
        binding
            .get("messageTargetStatus")
            .and_then(|value| value.as_str()),
        Some("probe-required")
    );
    assert_eq!(
        binding.get("nextAction").and_then(|value| value.as_str()),
        Some("probe-hidden-routable-child-before-replacement")
    );
}

#[test]
fn verified_canonical_target_presence_is_independent_of_model_evidence() {
    // Model evidence is deliberately not an input to host-target identity.
    // Runtime-profile admission remains a separate bootstrap/dispatch gate.
    assert!(super::verified_canonical_host_target_present(true));
    assert!(!super::verified_canonical_host_target_present(false));
}
