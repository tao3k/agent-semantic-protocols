#[test]
fn public_server_help_exposes_atomic_restart() {
    let help = super::runtime_server_command().render_help().to_string();
    for command in ["status", "stop", "restart"] {
        assert!(
            help.contains(command),
            "missing public lifecycle command {command}"
        );
    }
    for internal in ["reconcile", "daemon", "telemetry", "workspace"] {
        assert!(
            !help.contains(internal),
            "public lifecycle help leaked internal surface {internal}: {help}"
        );
    }
}

#[test]
fn public_server_lifecycle_rejects_workspace_identity() {
    use clap::Parser;

    for command in ["start", "status", "stop", "restart"] {
        let parsed = super::ServerArgs::try_parse_from([
            "asp server",
            command,
            "--workspace",
            "/tmp/project",
        ]);
        assert!(
            parsed.is_err(),
            "{command} must not expose workspace lifecycle identity"
        );
    }
}

#[test]
fn status_transport_failure_never_fabricates_stopped_lifecycle() {
    let failure = super::status_observation_failure(
        "Runtime Server control listener is unreachable: Operation not permitted (os error 1)",
    );

    assert!(failure.contains("reasonKind=transport-unavailable"));
    assert!(!failure.contains("state=stopped"));
}

#[test]
fn unauthenticated_status_failure_never_fabricates_stopped_lifecycle() {
    let failure = super::status_observation_failure("Runtime Server control listener closed early");

    assert!(failure.contains("reasonKind=runtime-status-observation-failed"));
    assert!(!failure.contains("state=stopped"));
}

#[test]
fn activation_authority_separates_operator_start_from_client_bootstrap() {
    use crate::server::runtime_server_wire_adapter::RuntimeServerActivationAuthority;

    assert!(
        RuntimeServerActivationAuthority::OperatorStart.is_explicit_operator_start(),
        "the public start command must clear an authoritative operator-stop tombstone"
    );
    assert!(
        !RuntimeServerActivationAuthority::ClientBootstrap.is_explicit_operator_start(),
        "automatic client bootstrap must not override an operator stop"
    );
}

#[tokio::test]
async fn operator_start_recovers_the_durable_applied_activation_without_pending_state() {
    use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
    use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent;
    use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactCandidateIdentityReceipt;

    let state_home = tempfile::tempdir().expect("temporary ASP State Home");
    let artifact_path = state_home.path().join("runtime/artifacts/candidate/asp");
    let stable_path = state_home.path().join("runtime/bin/asp");
    let digest = Blake3ContentDigest::from_bytes(b"operator-start-applied-activation");
    let event = RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        bundle_digest: digest.clone(),
        artifact_digest: digest.clone(),
        artifact_path: artifact_path.clone(),
        candidate_slot_path: artifact_path
            .parent()
            .expect("candidate artifact parent")
            .to_owned(),
        previous_artifact_digest: None,
        artifact_mode: "dev".to_owned(),
        published_at_unix_millis: 1,
        publication_nonce: "operator-start-applied".to_owned(),
        candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
            artifact_digest: digest,
            artifact_path,
            stable_path,
            artifact_mode: "dev".to_owned(),
            publication_nonce: "operator-start-applied".to_owned(),
        },
    };
    let applied = state_home.path().join("runtime/activation/applied.json");
    tokio::fs::create_dir_all(applied.parent().expect("applied activation parent"))
        .await
        .expect("create applied activation parent");
    tokio::fs::write(
        &applied,
        serde_json::to_vec(&event).expect("encode applied activation"),
    )
    .await
    .expect("write applied activation");

    let recovered = super::operator_start_activation_event(state_home.path())
        .await
        .expect("recover operator start activation")
        .expect("durable applied activation must be recoverable");
    assert_eq!(recovered, event);
}
