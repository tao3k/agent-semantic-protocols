// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::resident_transaction_identity_matches;
use super::runtime_activation_environment;

fn event() -> agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent
{
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"supervised readiness artifact",
        );
    let artifact_path = std::path::PathBuf::from("/runtime/artifacts/digest/asp");
    agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        activation_generation: 1,
        bundle_digest: artifact_digest.clone(),
        artifact_digest: artifact_digest.clone(),
        artifact_path: artifact_path.clone(),
        candidate_slot_path: std::path::PathBuf::from("/runtime/artifacts/bundles/candidate"),
        previous_artifact_digest: None,
        artifact_mode: "dev".to_owned(),
        published_at_unix_millis: 1,
        publication_nonce: "host-supervised-readiness".to_owned(),
        candidate_identity:
            agent_semantic_artifacts::runtime_artifact_activation::
                RuntimeArtifactCandidateIdentityReceipt {
                artifact_digest,
                artifact_path,
                stable_path: std::path::PathBuf::from("/runtime/bin/asp"),
                artifact_mode: "dev".to_owned(),
                publication_nonce: "host-supervised-readiness".to_owned(),
            },
    }
}

#[test]
fn runtime_activation_environment_has_no_client_bound_readiness_transport() {
    let environment =
        runtime_activation_environment(std::path::Path::new("/runtime-state"), &event());
    assert_eq!(environment.len(), 2);
    assert_eq!(
        environment[0],
        ("ASP_STATE_HOME".to_owned(), "/runtime-state".to_owned())
    );
    assert_eq!(environment[1].0, "ASP_RUNTIME_BINARY_CONTENT_DIGEST");
    assert!(
        environment
            .iter()
            .all(|(key, _)| key != "ASP_RUNTIME_ACTIVATION_READY_SOCKET")
    );
}

#[test]
fn healthy_endpoint_is_not_ready_before_transaction_identity_commit() {
    let event = event();
    assert!(!resident_transaction_identity_matches(
        &event,
        "different-publication",
        &event.artifact_digest,
    ));
    let different_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"different artifact",
        );
    assert!(!resident_transaction_identity_matches(
        &event,
        &event.publication_nonce,
        &different_digest,
    ));
    assert!(resident_transaction_identity_matches(
        &event,
        &event.publication_nonce,
        &event.artifact_digest,
    ));
}
