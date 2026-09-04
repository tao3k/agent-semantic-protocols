use std::path::Path;

use crate::runtime_serving_endpoint::resolve_runtime_serving_endpoint;

const DIGEST: &str = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn endpoint_json(state_home: &Path, artifact_path: &Path) -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-endpoint",
        "schemaVersion": "1",
        "binaryContentDigest": DIGEST,
        "runtimeGenerationDigest": DIGEST,
        "schemaDigest": DIGEST,
        "transportContractDigest": DIGEST,
        "ownerEpoch": 1,
        "ownerProcessId": 42,
        "runtimeArtifactPath": artifact_path,
        "runtimeBinaryIdentity": {"kind":"content", "identity":{"digest": DIGEST}},
        "monitorCapability": true,
        "observedRuntimeBinaryIdentity": {"kind":"content", "identity":{"digest": DIGEST}},
        "artifactMode": "dev",
        "artifactCatalogDigest": DIGEST,
        "bindingToken": "binding",
        "controlEndpoint": {"transport":"loopback-tcp", "address":"127.0.0.1", "port": 5001},
        "dataEndpoint": {"transport":"loopback-tcp", "address":"127.0.0.1", "port": 5002},
        "providerEndpoint": {"transport":"loopback-tcp", "address":"127.0.0.1", "port": 5003},
        "workspaceStorePath": state_home.join("runtime/workspaces"),
        "statusMemoryPath": state_home.join("runtime/status.memory")
    })
}

#[tokio::test]
async fn serving_endpoint_requires_one_owner_activation_and_endpoint_identity() {
    let state_home = tempfile::tempdir().expect("State Home");
    let artifact = state_home.path().join("runtime/bin/asp");
    std::fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    std::fs::write(&artifact, b"fixture").expect("artifact");
    let owner_path = state_home.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(owner_path.parent().expect("owner parent")).expect("owner dir");
    std::fs::write(
        &owner_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-owner-spawn.v1",
            "schemaVersion": "1", "processId": 42, "nonce": "owner",
            "stateHome": state_home.path(), "publicationNonce": "publication",
            "launcherArtifactPath": artifact, "launcherArtifactDigest": DIGEST,
            "spawnArgv": ["asp", "server", "daemon"], "previousServingDigest": null,
            "previousOwnerEpoch": null
        }))
        .expect("encode owner"),
    )
    .expect("owner receipt");
    let activation_path = state_home.path().join("runtime/activation/applied.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("activation dir");
    std::fs::write(
        &activation_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId":"agent.semantic-protocols.runtime-artifact-activation", "schemaVersion":1,
            "bundleDigest":DIGEST, "artifactDigest":DIGEST, "artifactPath":artifact,
            "candidateSlotPath":state_home.path().join("runtime/artifacts/candidate"),
            "previousArtifactDigest":null, "artifactMode":"dev", "publishedAtUnixMillis":1,
            "publicationNonce":"publication", "candidateIdentity":{
              "artifactDigest":DIGEST, "artifactPath":artifact, "stablePath":artifact,
              "artifactMode":"dev", "publicationNonce":"publication"
            }
        }))
        .expect("encode activation"),
    )
    .expect("activation receipt");
    let endpoint_path = state_home
        .path()
        .join("runtime/resident/current/endpoint.json");
    std::fs::create_dir_all(endpoint_path.parent().expect("endpoint parent"))
        .expect("endpoint dir");
    std::fs::write(
        &endpoint_path,
        serde_json::to_vec(&endpoint_json(state_home.path(), &artifact)).expect("encode endpoint"),
    )
    .expect("endpoint receipt");

    let endpoint = resolve_runtime_serving_endpoint(state_home.path())
        .await
        .expect("verified endpoint");
    assert_eq!(endpoint.data_socket_addr.port(), 5002);
    assert_eq!(endpoint.publication_nonce, "publication");
}
