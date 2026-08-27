use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use serde_json::json;

#[tokio::test]
async fn supervisor_startup_reader_migrates_old_endpoint_before_typed_decode() {
    let state_home = tempfile::tempdir().expect("temporary State Home");
    let artifact = state_home.path().join("runtime/bin/asp");
    tokio::fs::create_dir_all(artifact.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&artifact, b"asp-fixture").await.unwrap();
    let digest = Blake3ContentDigest::from_bytes(b"asp-fixture");
    let mut endpoint = agent_semantic_client_db::prepare_runtime_server_endpoint(
        state_home.path(),
        &artifact,
        &digest,
        "dev",
        &Blake3ContentDigest::from_bytes(b"catalog").to_string(),
        1,
        "startup-migration",
    )
    .await
    .unwrap();
    endpoint.owner_process_id = std::process::id();
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            state_home.path(),
        )
        .await
        .unwrap();
    agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint(
        &endpoint_path,
        &endpoint,
    )
    .await
    .unwrap();

    let mut value: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&endpoint_path).await.unwrap()).unwrap();
    for field in ["runtimeBinaryIdentity", "observedRuntimeBinaryIdentity"] {
        value[field] = json!({
            "kind": "content",
            "identity": {
                "value": digest.to_string(),
                "algorithm": "blake3-256"
            }
        });
    }
    for field in [
        "binaryContentDigest",
        "runtimeGenerationDigest",
        "schemaDigest",
    ] {
        value.as_object_mut().unwrap().remove(field);
    }
    tokio::fs::write(&endpoint_path, serde_json::to_vec_pretty(&value).unwrap())
        .await
        .unwrap();

    let migrated =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home.path(),
        )
        .await
        .unwrap()
        .expect("migrated endpoint");
    assert_eq!(migrated.binary_content_digest, digest.to_string());
    assert_canonical(&migrated.runtime_generation_digest);
    assert_canonical(&migrated.schema_digest);

    let rewritten: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(endpoint_path).await.unwrap()).unwrap();
    assert_eq!(rewritten["binaryContentDigest"], digest.to_string());
}

fn assert_canonical(value: &str) {
    let suffix = value.strip_prefix("blake3-256:").expect("typed digest");
    assert_eq!(suffix.len(), 64);
    assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
