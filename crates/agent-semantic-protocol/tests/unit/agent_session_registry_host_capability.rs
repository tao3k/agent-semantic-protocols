use super::{
    HOST_ACK_SOURCE, HOST_ROUTE_PROBE_SOURCE, HOST_TREE_SCHEMA_ID, HOST_TREE_SCHEMA_VERSION,
    HOST_TREE_SOURCE, HostResidentTargetObservation,
    consume_fresh_unroutable_resident_target_observation, fresh_host_resident_target_observation,
    write_host_tree_observation,
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

#[test]
fn absent_observation_cannot_be_consumed_as_replacement_lease() {
    let root = std::env::temp_dir().join(format!("asp-host-probe-lease-{}", std::process::id()));
    let registry = agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root(
        root.join("state"),
    )
    .expect("create registry");
    let mut observation = HostResidentTargetObservation {
        schema_id: HOST_TREE_SCHEMA_ID.to_string(),
        schema_version: HOST_TREE_SCHEMA_VERSION.to_string(),
        root_session_id: "root".to_string(),
        resident_name: "asp-explore".to_string(),
        target_status: "absent".to_string(),
        canonical_target: None,
        identity_status: "unverified".to_string(),
        source: HOST_TREE_SOURCE.to_string(),
        probe_evidence_ref: None,
        observed_at: 10,
        expires_at: 20,
    };

    write_host_tree_observation(&registry, &observation).expect("write absent observation");
    assert!(
        !consume_fresh_unroutable_resident_target_observation(
            &registry,
            "root",
            "asp-explore",
            15,
        )
        .expect("reject absent replacement lease")
    );
    assert!(
        fresh_host_resident_target_observation(&registry, "root", "asp-explore", 15)
            .expect("read absent observation")
            .is_some()
    );

    observation.target_status = "unroutable".to_string();
    observation.canonical_target = Some("/root/asp_explorer".to_string());
    observation.source = HOST_ROUTE_PROBE_SOURCE.to_string();
    observation.probe_evidence_ref = Some("canonical-followup-not-found:1".to_string());
    write_host_tree_observation(&registry, &observation).expect("write unroutable observation");
    assert!(
        consume_fresh_unroutable_resident_target_observation(&registry, "root", "asp-explore", 15,)
            .expect("consume unroutable replacement lease")
    );
    assert!(
        !consume_fresh_unroutable_resident_target_observation(
            &registry,
            "root",
            "asp-explore",
            15,
        )
        .expect("fence duplicate replacement lease")
    );
    let _ = std::fs::remove_dir_all(root);
}
