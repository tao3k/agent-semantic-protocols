// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_db::runtime_server_control::read_supervisor_endpoint;
use serde_json::Value;
use serde_json::json;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;

fn old_v1_endpoint(owner_process_id: u32) -> Value {
    let digest = format!("blake3-256:{}", "a".repeat(64));
    json!({
        "schemaId": "agent.semantic-protocols.runtime-server-endpoint",
        "schemaVersion": "1",
        "transportContractDigest": "test-transport-contract",
        "ownerEpoch": 7,
        "ownerProcessId": owner_process_id,
        "runtimeArtifactPath": "/tmp/asp-artifacts/active/asp",
        "runtimeBinaryIdentity": {
            "kind": "content",
            "identity": { "value": digest, "algorithm": "blake3-256" }
        },
        "monitorCapability": true,
        "observedRuntimeBinaryIdentity": {
            "kind": "content",
            "identity": { "value": digest, "algorithm": "blake3-256" }
        },
        "artifactMode": "dev",
        "artifactCatalogDigest": digest,
        "bindingToken": "owner-nonce",
        "socketPath": "/tmp/asp-runtime/control.sock",
        "dataPlaneSocketPath": "/tmp/asp-runtime/data.sock",
        "providerPlaneSocketPath": "/tmp/asp-runtime/provider.sock",
        "clientHttpEndpoint": "http://127.0.0.1:43191",
        "workspaceStorePath": "/tmp/asp-runtime/workspace",
        "statusMemoryPath": "/tmp/asp-runtime/status"
    })
}

#[tokio::test]
async fn old_schema_v1_endpoint_migrates_identity_without_stopping_healthy_owner() {
    let temporary = tempfile::tempdir().expect("temporary endpoint home");
    let path = temporary.path().join("endpoint.v1.json");
    let owner_process_id = std::process::id();
    tokio::fs::write(
        &path,
        serde_json::to_vec(&old_v1_endpoint(owner_process_id)).expect("encode old endpoint"),
    )
    .await
    .expect("write old endpoint");
    tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .await
        .expect("set old endpoint permissions");

    let endpoint = read_supervisor_endpoint(&path)
        .await
        .expect("migrate healthy endpoint");
    assert_eq!(endpoint.owner_process_id, owner_process_id);
    assert_eq!(
        endpoint.binary_content_digest,
        endpoint.runtime_binary_identity.content_digest().as_str()
    );
    assert!(
        endpoint
            .runtime_generation_digest
            .starts_with("blake3-256:")
    );
    assert!(endpoint.schema_digest.starts_with("blake3-256:"));

    let rewritten: Value = serde_json::from_slice(
        &tokio::fs::read(&path)
            .await
            .expect("read rewritten endpoint"),
    )
    .expect("decode rewritten endpoint");
    assert!(rewritten.get("binaryContentDigest").is_some());
    assert!(rewritten.get("runtimeGenerationDigest").is_some());
    assert!(rewritten.get("schemaDigest").is_some());
    let metadata = tokio::fs::symlink_metadata(&path)
        .await
        .expect("migrated endpoint metadata");
    assert!(metadata.file_type().is_file());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.mode() & 0o777, 0o600);
}

#[tokio::test]
async fn writable_schema_v1_endpoint_is_not_canonicalized_or_decoded() {
    let temporary = tempfile::tempdir().expect("temporary endpoint home");
    let path = temporary.path().join("endpoint.v1.json");
    tokio::fs::write(
        &path,
        serde_json::to_vec(&old_v1_endpoint(std::process::id())).expect("encode old endpoint"),
    )
    .await
    .expect("write old endpoint");
    tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o622))
        .await
        .expect("set writable endpoint permissions");

    let error = read_supervisor_endpoint(&path)
        .await
        .expect_err("group/world-writable endpoint must fail closed");
    assert!(error.contains("not a non-writable, non-symlink current-UID file"));
    assert_eq!(
        tokio::fs::symlink_metadata(&path)
            .await
            .expect("endpoint metadata")
            .mode()
            & 0o777,
        0o622
    );
}

#[tokio::test]
async fn old_schema_v1_endpoint_without_owner_authority_is_typed_incomplete() {
    let temporary = tempfile::tempdir().expect("temporary endpoint home");
    let path = temporary.path().join("endpoint.v1.json");
    tokio::fs::write(
        &path,
        serde_json::to_vec(&old_v1_endpoint(0)).expect("encode old endpoint"),
    )
    .await
    .expect("write old endpoint");

    let error = read_supervisor_endpoint(&path)
        .await
        .expect_err("missing owner authority must fail");
    assert!(error.contains("reasonKind=runtime-server-endpoint-identity-incomplete"));
}
