use super::{HOST_ACK_SOURCE, HOST_TREE_SCHEMA_ID, HostResidentTargetObservation, SCHEMA_VERSION};

#[test]
fn host_resident_target_observation_accepts_followup_ack_source() {
    let observation = HostResidentTargetObservation {
        schema_id: HOST_TREE_SCHEMA_ID.to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        root_session_id: "root".to_string(),
        resident_name: "asp-explore".to_string(),
        target_status: "present".to_string(),
        canonical_target: Some("/root/asp_explorer".to_string()),
        identity_status: "verified".to_string(),
        source: HOST_ACK_SOURCE.to_string(),
        observed_at: 10,
        expires_at: 20,
    };

    assert!(observation.is_fresh_for("root", "asp-explore", 15));
}
