use super::validate_activation_ready_binding;

fn event() -> agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent
{
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"ready binding artifact",
        );
    let artifact_path = std::path::PathBuf::from("/runtime/artifacts/digest/asp");
    agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        bundle_digest: artifact_digest.clone(),
        artifact_digest: artifact_digest.clone(),
        artifact_path: artifact_path.clone(),
        candidate_slot_path: std::path::PathBuf::from("/runtime/resident/candidate"),
        previous_artifact_digest: None,
        artifact_mode: "dev".to_owned(),
        published_at_unix_millis: 1,
        publication_nonce: "ready-binding".to_owned(),
        candidate_identity:
            agent_semantic_artifacts::runtime_artifact_activation::
                RuntimeArtifactCandidateIdentityReceipt {
                artifact_digest,
                artifact_path,
                stable_path: std::path::PathBuf::from("/runtime/bin/asp"),
                artifact_mode: "dev".to_owned(),
                publication_nonce: "ready-binding".to_owned(),
            },
    }
}

fn spawn(
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
) -> agent_semantic_client_db::RuntimeServerSpawnReceipt {
    agent_semantic_client_db::RuntimeServerSpawnReceipt {
        schema_id: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION
            .to_owned(),
        process_id: 41,
        nonce: "owner-41".to_owned(),
        state_home: "/runtime-state".to_owned(),
        publication_nonce: event.publication_nonce.clone(),
        launcher_artifact_path: event.artifact_path.display().to_string(),
        launcher_artifact_digest: event.artifact_digest.clone(),
        spawn_argv: vec!["server".to_owned(), "daemon".to_owned()],
        previous_serving_digest: None,
        previous_owner_epoch: None,
    }
}

fn ready(
    event: &agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    spawn: &agent_semantic_client_db::RuntimeServerSpawnReceipt,
) -> agent_semantic_client_db::RuntimeServerActivationReadyReceipt {
    agent_semantic_client_db::RuntimeServerActivationReadyReceipt {
        schema_id: "agent.semantic-protocols.runtime-activation-ready-receipt".to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        publication_nonce: event.publication_nonce.clone(),
        artifact_digest: event.artifact_digest.clone(),
        owner_epoch: 41,
        launcher_receipt_digest:
            agent_semantic_client_db::runtime_server_lifecycle::spawn_receipt_digest(spawn)
                .expect("launcher receipt digest"),
    }
}

#[test]
fn ready_receipt_requires_exact_nonce_digest_and_launcher_binding() {
    let event = event();
    let spawn = spawn(&event);
    let receipt = ready(&event, &spawn);
    validate_activation_ready_binding(&receipt, &event, &spawn).expect("matching ready authority");

    let mut wrong_nonce = receipt.clone();
    wrong_nonce.publication_nonce = "wrong-publication".to_owned();
    assert!(
        validate_activation_ready_binding(&wrong_nonce, &event, &spawn)
            .unwrap_err()
            .contains("ready-authority-binding-mismatch")
    );

    let mut wrong_digest = receipt.clone();
    wrong_digest.artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"wrong artifact",
        );
    assert!(
        validate_activation_ready_binding(&wrong_digest, &event, &spawn)
            .unwrap_err()
            .contains("ready-authority-binding-mismatch")
    );

    let mut wrong_launcher = receipt;
    wrong_launcher.launcher_receipt_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"wrong launcher receipt",
        );
    assert!(
        validate_activation_ready_binding(&wrong_launcher, &event, &spawn)
            .unwrap_err()
            .contains("ready-authority-binding-mismatch")
    );
}

#[tokio::test]
async fn ready_channel_cannot_replace_the_canonical_applied_transaction() {
    let temporary = tempfile::tempdir().expect("temporary ready authority");
    let event = event();
    let mut spawn = spawn(&event);
    spawn.state_home = temporary.path().display().to_string();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        temporary.path(),
        &spawn,
    )
    .await
    .expect("publish owner-spawn authority");
    let mut listener =
        agent_semantic_client_db::runtime_server_lifecycle::bind_activation_ready_listener(
            temporary.path(),
            &event.publication_nonce,
        )
        .await
        .expect("bind ready listener");
    let receipt = ready(&event, &spawn);
    agent_semantic_client_db::runtime_server_lifecycle::publish_activation_ready(
        listener.path(),
        &receipt,
    )
    .await
    .expect("publish isolated ready signal");
    let received = listener.receive().await.expect("receive ready signal");
    validate_activation_ready_binding(&received, &event, &spawn).expect("ready signal binding");

    let error = agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
        temporary.path(),
    )
    .await
    .expect_err("ready signal without applied commit must remain non-authoritative");
    assert!(
        error.contains("applied activation"),
        "unexpected error: {error}"
    );
}
