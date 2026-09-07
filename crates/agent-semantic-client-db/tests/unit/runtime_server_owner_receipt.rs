// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::RuntimeServerSpawnReceiptRead;
use agent_semantic_client_db::runtime_server_owner_receipt::decode_runtime_server_spawn_receipt;

#[test]
fn generation_bound_owner_observation_discards_activation_generation_and_becomes_stale() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-owner-spawn.v1",
        "schemaVersion": "1",
        "processId": 986,
        "nonce": "owner-986",
        "stateHome": "/state-home",
        "activationGeneration": 86,
        "launcherArtifactPath": "/runtime/bin/asp",
        "launcherArtifactDigest": format!("blake3-256:{}", "a".repeat(64)),
        "spawnArgv": ["server", "daemon"],
        "previousServingDigest": null,
        "previousOwnerEpoch": null
    }))
    .expect("encode generation-bound observation");

    match decode_runtime_server_spawn_receipt(&bytes).expect("decode stale observation") {
        RuntimeServerSpawnReceiptRead::Stale(stale) => assert_eq!(
            stale.reason_kind,
            "runtime-server-owner-spawn-activation-generation-discarded"
        ),
        RuntimeServerSpawnReceiptRead::Current(_) => {
            panic!("discarded activation generation cannot become current authority")
        }
    }
}

#[test]
fn arbitrary_partial_launcher_authority_remains_fail_closed() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-owner-spawn.v1",
        "schemaVersion": "1",
        "processId": 986,
        "nonce": "owner-986",
        "stateHome": "/state-home",
        "launcherArtifactPath": "/runtime/bin/asp"
    }))
    .expect("encode partial observation");

    let error = decode_runtime_server_spawn_receipt(&bytes)
        .expect_err("partial launcher authority must fail closed");
    assert!(error.contains("runtime-server-owner-spawn-partial-launcher-authority"));
}
