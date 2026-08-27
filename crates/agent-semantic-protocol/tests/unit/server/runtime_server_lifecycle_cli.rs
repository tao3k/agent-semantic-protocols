#[test]
fn public_server_help_exposes_observation_and_operator_stop_without_restart() {
    let help = super::runtime_server_command().render_help().to_string();
    for command in ["status", "stop"] {
        assert!(
            help.contains(command),
            "missing public lifecycle command {command}"
        );
    }
    for internal in ["restart", "reconcile", "daemon", "telemetry", "workspace"] {
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
    use agent_semantic_artifacts::runtime_artifact_publication::{
        RuntimeArtifactActivationEvent, RuntimeArtifactCandidateIdentityReceipt,
    };

    let state_home = tempfile::tempdir().expect("temporary ASP State Home");
    let artifact_path = state_home.path().join("runtime/artifacts/candidate/asp");
    let stable_path = state_home.path().join("runtime/bin/asp");
    let digest = Blake3ContentDigest::from_bytes(b"operator-start-applied-activation");
    let event = RuntimeArtifactActivationEvent {
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
        activation_generation: 19,
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

#[test]
fn query_scope_derives_identity_without_catalog_or_bootstrap() {
    let project_root = std::env::current_dir()
        .expect("current project root")
        .canonicalize()
        .expect("canonical project root");
    let expected = agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
        .expect("derive workspace identity");
    let (actual, canonical_root) =
        super::runtime_server_query_workspace_scope(&project_root).expect("derive query scope");
    assert_eq!(actual, expected);
    assert_eq!(canonical_root, project_root);
}

#[test]
fn client_bootstrap_continues_only_for_an_already_resident_runtime() {
    use agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome;

    let authority =
        agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapAuthority {
            cwd: "/workspace".into(),
            executable_path: "/workspace/target/debug/asp".into(),
            state_home: "/state".into(),
            state_home_source:
                agent_semantic_runtime::state_core::StateHomeResolutionSource::HomeDefault,
            asp_state_home_present: false,
            home_present: true,
            pending_activation_path: "/state/runtime/resident/active/activation.json".into(),
            applied_activation_path: "/state/runtime/activation/applied.json".into(),
            runtime_endpoint_path: "/state/runtime/server/endpoint.v1.json".into(),
        };

    assert!(super::runtime_server_client_bootstrap_continues(
        super::RuntimeServerClientBootstrapDisposition::Continue
    ));
    assert!(!super::runtime_server_client_bootstrap_continues(
        super::RuntimeServerClientBootstrapDisposition::Terminal
    ));
    assert_eq!(
        super::runtime_server_client_bootstrap_receipt(
            SupervisorOutcome::AlreadyResident,
            7,
            "blake3-256:test".to_owned(),
            &authority,
        ),
        None
    );
    for (outcome, state, reason_kind) in [
        (
            SupervisorOutcome::SpawnAccepted,
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting,
            "runtime-server-activation-spawn-accepted",
        ),
        (
            SupervisorOutcome::OwnerStale,
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Degraded,
            "runtime-server-owner-stale",
        ),
        (
            SupervisorOutcome::Failed,
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Degraded,
            "runtime-server-supervisor-failed",
        ),
    ] {
        let terminal = super::runtime_server_client_bootstrap_receipt(
            outcome,
            7,
            "blake3-256:test".to_owned(),
            &authority,
        )
        .expect("non-resident supervisor outcome must terminalize the client bootstrap");
        assert_eq!(terminal.state, state);
        assert_eq!(terminal.reason_kind, reason_kind);
        assert_eq!(terminal.activation_generation, Some(7));
        assert_eq!(terminal.artifact_digest, Some("blake3-256:test".to_owned()));
        assert!(!terminal.recommended_next.is_empty());
        assert_eq!(terminal.authority, authority);
    }
}
