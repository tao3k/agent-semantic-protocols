use super::{
    HOST_ACK_SOURCE, HOST_ROUTE_PROBE_SOURCE, HOST_TREE_SCHEMA_ID, HOST_TREE_SCHEMA_VERSION,
    HostResidentTargetObservation,
};

#[test]
fn host_resident_target_observation_accepts_followup_ack_source() {
    let observation = HostResidentTargetObservation {
        schema_id: HOST_TREE_SCHEMA_ID.to_string(),
        schema_version: HOST_TREE_SCHEMA_VERSION.to_string(),
        root_session_id: "root".to_string(),
        resident_name: "asp-explore".to_string(),
        target_status: "present".to_string(),
        canonical_target: Some("/root/asp_explorer".to_string()),
        identity_status: "verified".to_string(),
        source: HOST_ACK_SOURCE.to_string(),
        probe_evidence_ref: None,
        observed_at: 10,
        expires_at: 20,
    };

    assert!(observation.is_fresh_for("root", "asp-explore", 15));
}

#[test]
fn registered_resident_target_rejects_temporary_same_profile_agent() {
    assert!(crate::command::agent_session_registry::agent_session_registry_host_capability::registered_resident_target_matches(
        "/root/asp_explorer",
        "asp_explorer"
    ));
    assert!(!crate::command::agent_session_registry::agent_session_registry_host_capability::registered_resident_target_matches(
        "/root/policy_owner_explorer",
        "asp_explorer"
    ));
    assert!(crate::command::agent_session_registry::agent_session_registry_host_capability::registered_resident_target_matches(
        "/root/asp_testing",
        "asp_testing"
    ));
}

#[test]
fn explicit_child_registration_skips_rollout_adoption_scan() {
    use crate::command::agent_session_registry::agent_session_registry_commands::should_adopt_reusable_rollout_session;

    assert!(!should_adopt_reusable_rollout_session(
        Some("child-session"),
        false
    ));
    assert!(!should_adopt_reusable_rollout_session(None, true));
    assert!(should_adopt_reusable_rollout_session(None, false));
}

#[test]
fn unroutable_host_target_requires_canonical_probe_evidence() {
    let mut observation = HostResidentTargetObservation {
        schema_id: HOST_TREE_SCHEMA_ID.to_string(),
        schema_version: HOST_TREE_SCHEMA_VERSION.to_string(),
        root_session_id: "root".to_string(),
        resident_name: "asp-explore".to_string(),
        target_status: "unroutable".to_string(),
        canonical_target: Some("/root/asp_explorer".to_string()),
        identity_status: "unverified".to_string(),
        source: HOST_ROUTE_PROBE_SOURCE.to_string(),
        probe_evidence_ref: None,
        observed_at: 10,
        expires_at: 20,
    };

    assert!(!observation.is_fresh_for("root", "asp-explore", 15));
    observation.probe_evidence_ref = Some("canonical-followup-not-found:1".to_string());
    assert!(observation.is_fresh_for("root", "asp-explore", 15));
}
