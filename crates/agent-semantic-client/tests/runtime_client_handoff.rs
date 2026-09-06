// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client::{AspClient, AspClientRuntimeHandoff};
use agent_semantic_client_db::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt;

fn transaction_receipt() -> RuntimeServerResidentTransactionReceipt {
    serde_json::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-resident-transaction-receipt",
        "schemaVersion": "1",
        "state": "ready",
        "publicationNonce": "publication-current",
        "launcherArtifactPath": "/runtime/artifacts/current/asp",
        "launcherArtifactDigest": format!("blake3-256:{}", "a".repeat(64)),
        "spawnArgv": ["server", "daemon"],
        "appliedArtifactDigest": format!("blake3-256:{}", "a".repeat(64)),
        "appliedPublicationNonce": "publication-current",
        "endpointOwnerEpoch": 7,
        "endpointBinaryContentDigest": format!("blake3-256:{}", "a".repeat(64)),
        "endpointRuntimeGenerationDigest": format!("blake3-256:{}", "b".repeat(64)),
        "controlEndpoint": {"transport": "loopback-tcp", "address": "127.0.0.1", "port": 41001},
        "dataEndpoint": {"transport": "loopback-tcp", "address": "127.0.0.1", "port": 41002},
        "providerEndpoint": {"transport": "loopback-tcp", "address": "127.0.0.1", "port": 41003},
        "previousServingDigest": null,
        "previousOwnerEpoch": null,
        "previousDrainState": "not-running"
    }))
    .expect("valid resident transaction fixture")
}

#[test]
fn ready_transaction_mints_exact_runtime_client_handoff() {
    let receipt = transaction_receipt();
    let handoff = AspClientRuntimeHandoff::try_from(&receipt).expect("exact ready handoff");

    assert_eq!(handoff.control_socket_addr().to_string(), "127.0.0.1:41001");
    assert_eq!(handoff.data_socket_addr().to_string(), "127.0.0.1:41002");
    assert_eq!(
        handoff.provider_socket_addr().to_string(),
        "127.0.0.1:41003"
    );
    assert_eq!(handoff.publication_nonce(), "publication-current");
    assert_eq!(
        handoff.artifact_digest(),
        &receipt.endpoint_binary_content_digest
    );
    assert_eq!(handoff.owner_epoch(), 7);
    assert_eq!(
        handoff.runtime_generation_digest(),
        receipt.endpoint_runtime_generation_digest
    );
}

#[test]
fn runtime_client_handoff_rejects_cross_artifact_receipt() {
    let mut receipt = transaction_receipt();
    receipt.endpoint_binary_content_digest =
        serde_json::from_value(serde_json::json!(format!("blake3-256:{}", "c".repeat(64))))
            .expect("valid digest");

    let error = AspClientRuntimeHandoff::try_from(&receipt).expect_err("cross-artifact reject");
    assert!(error.contains("reasonKind=runtime-client-handoff-identity-mismatch"));
}

#[test]
fn runtime_client_handoff_rejects_cross_publication_receipt() {
    let mut receipt = transaction_receipt();
    receipt.applied_publication_nonce = "publication-stale".to_owned();

    let error = AspClientRuntimeHandoff::try_from(&receipt).expect_err("cross-publication reject");
    assert!(error.contains("reasonKind=runtime-client-handoff-identity-mismatch"));
}

#[tokio::test]
async fn pinned_runtime_handoff_never_rederives_state_home_endpoint() {
    let handoff =
        AspClientRuntimeHandoff::try_from(&transaction_receipt()).expect("exact ready handoff");
    let client = AspClient::new_from_runtime_handoff(
        "/state-home-that-does-not-exist",
        "/workspace-that-does-not-exist",
        handoff,
    );

    let error = client
        .backpressure_probe()
        .await
        .expect_err("fixture endpoint is intentionally not listening");
    assert!(error.contains("Runtime serving endpoint was content-proven"));
    assert!(!error.contains("reasonKind=transport-unavailable"));
    assert!(!error.contains("endpoint.v1.json"));
}

#[tokio::test]
async fn unpinned_client_requests_runtime_handoff_instead_of_resolving_endpoint_path() {
    let client = AspClient::new(
        "/state-home-that-does-not-exist",
        "/workspace-that-does-not-exist",
    );

    let error = client
        .backpressure_probe()
        .await
        .expect_err("no resident transaction exists for the fixture");
    assert!(error.contains("reasonKind=runtime-client-handoff-unavailable"));
    assert!(!error.contains("reasonKind=transport-unavailable"));
}
