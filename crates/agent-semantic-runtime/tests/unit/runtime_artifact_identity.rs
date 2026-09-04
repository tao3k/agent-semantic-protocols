use super::RuntimeArtifactIdentityReceipt;
use super::SCHEMA_ID;
use super::SCHEMA_VERSION;
use super::admit_runtime_invoker;
use super::executable_identity;
use super::read_runtime_artifact_identity;
use super::runtime_artifact_identity_path;
use super::runtime_artifact_source_generation;

#[test]
fn identity_path_is_version_neutral_and_binary_scoped() {
    let state_home = std::path::Path::new("/state");
    assert_eq!(
        runtime_artifact_identity_path(state_home, "asp").expect("identity path"),
        state_home.join("runtime/artifact-identities/asp.json")
    );
    assert!(runtime_artifact_identity_path(state_home, "../asp").is_err());
}

#[test]
fn source_generation_is_stable_until_the_source_changes() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("asp");
    std::fs::write(&source, b"generation-a").expect("write first generation");
    let first = runtime_artifact_source_generation(&source).expect("first generation");
    let current = runtime_artifact_source_generation(&source).expect("current generation");
    assert_eq!(first, current);

    std::fs::write(&source, b"generation-b").expect("write second generation");
    let second = runtime_artifact_source_generation(&source).expect("second generation");
    assert_ne!(first, second);
}

#[tokio::test]
async fn rejects_identity_with_wrong_schema_version() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = runtime_artifact_identity_path(root.path(), "asp").expect("identity path");
    tokio::fs::create_dir_all(path.parent().expect("identity parent"))
        .await
        .expect("create identity parent");
    tokio::fs::write(
            &path,
            br#"{"schemaId":"agent.semantic-protocols.runtime-artifact-identity","schemaVersion":"2","artifactKind":"asp","artifactMode":"dev","stablePath":"/state/runtime/bin/asp","sourcePath":"/checkout/target/debug/asp","artifactDigest":"digest"}"#,
        )
        .await
        .expect("write invalid identity");
    assert!(
        read_runtime_artifact_identity(root.path(), "asp")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_non_content_or_mismatched_identity() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = runtime_artifact_identity_path(root.path(), "asp").expect("identity path");
    tokio::fs::create_dir_all(path.parent().expect("identity parent"))
        .await
        .expect("create identity parent");
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"fixture-digest",
    );
    tokio::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-artifact-identity",
            "schemaVersion": "1",
            "artifactKind": "asp",
            "artifactMode": "dev",
            "stablePath": root.path().join("runtime/bin/asp"),
            "sourcePath": root.path().join("checkout/target/debug/asp"),
            "sourceGeneration": format!("blake3-256:{}", "b".repeat(64)),
            "sourceGenerationAlgorithm": "filesystem-generation-v1",
            "artifactDigest": digest.clone(),
            "identityKind": "source-generation",
            "identityValue": digest,
            "identityAlgorithm": "metadata"
        }))
        .expect("encode invalid identity"),
    )
    .await
    .expect("write invalid identity");
    let error = read_runtime_artifact_identity(root.path(), "asp")
        .await
        .expect_err("non-content identity must fail closed");
    assert!(error.contains("content contract mismatch"));
}

#[test]
#[cfg(unix)]
fn developer_invoker_requires_the_stable_entry_to_resolve_directly_to_source() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("checkout/target/debug/asp");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("mkdir");
    std::fs::write(&source, b"developer-runtime").expect("source");
    let stable = root.path().join("runtime/bin/asp");
    std::fs::create_dir_all(stable.parent().expect("stable parent")).expect("mkdir");
    std::os::unix::fs::symlink(&source, &stable).expect("direct Developer link");
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"developer-runtime",
    );
    let identity = executable_identity(&source, &digest).expect("source identity");
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind: "asp".to_owned(),
        artifact_mode: "dev".to_owned(),
        stable_path: stable.clone(),
        source_path: source.canonicalize().expect("canonical source"),
        source_generation: runtime_artifact_source_generation(&source).expect("generation"),
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
        artifact_digest: digest.to_string(),
        identity_kind: "content".to_owned(),
        identity_value: digest,
        identity_algorithm: "blake3-256".to_owned(),
        source_executable_identity: Some(identity.clone()),
        active_executable_identity: Some(identity),
    };
    let admission = admit_runtime_invoker(&source, &receipt, &stable);
    assert!(
        admission.is_ok(),
        "Developer admission failed: {admission:?}"
    );

    std::fs::remove_file(&stable).expect("remove direct link");
    let copied = root.path().join("runtime/artifacts/copied-asp");
    std::fs::create_dir_all(copied.parent().expect("copy parent")).expect("mkdir");
    std::fs::copy(&source, &copied).expect("copy artifact");
    std::os::unix::fs::symlink(&copied, &stable).expect("artifact-backed link");
    assert!(
        admit_runtime_invoker(&source, &receipt, &stable)
            .expect_err("Developer mode must reject artifact copies")
            .contains("developer-direct-link-drift")
    );
}

#[test]
fn invoker_admission_accepts_source_and_active_and_rejects_rebuild() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("source-asp");
    std::fs::write(&source, b"runtime").expect("source");
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"runtime",
    );
    let active = root
        .path()
        .join("runtime/artifacts/blake3-256")
        .join(blake3::hash(b"runtime").to_hex().as_str())
        .join("asp");
    std::fs::create_dir_all(active.parent().expect("active parent")).expect("mkdir");
    std::fs::copy(&source, &active).expect("active");
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind: "asp".to_owned(),
        artifact_mode: "release".to_owned(),
        stable_path: root.path().join("runtime/bin/asp"),
        source_path: source.canonicalize().expect("canonical source"),
        source_generation: runtime_artifact_source_generation(&source).expect("generation"),
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
        artifact_digest: digest.to_string(),
        identity_kind: "content".to_owned(),
        identity_value: digest.clone(),
        identity_algorithm: "blake3-256".to_owned(),
        source_executable_identity: Some(
            executable_identity(&source, &digest).expect("source identity"),
        ),
        active_executable_identity: Some(
            executable_identity(&active, &digest).expect("active identity"),
        ),
    };
    let admission = admit_runtime_invoker(&source, &receipt, &active);
    assert!(
        admission.is_ok(),
        "unexpected admission failure: {admission:?}; active={active:?}; canonical_active={:?}",
        std::fs::canonicalize(&active)
    );
    assert!(admit_runtime_invoker(&active, &receipt, &active).is_ok());
    std::fs::write(&source, b"rebuilt").expect("rebuild");
    assert!(admit_runtime_invoker(&source, &receipt, &active).is_err());
}

#[test]
fn invoker_admission_rejects_missing_identity_and_active_digest_drift() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("source-asp");
    std::fs::write(&source, b"runtime").expect("source");
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"runtime",
    );
    let active = root
        .path()
        .join("runtime/artifacts/blake3-256")
        .join(blake3::hash(b"runtime").to_hex().as_str())
        .join("asp");
    std::fs::create_dir_all(active.parent().expect("active parent")).expect("mkdir");
    std::fs::copy(&source, &active).expect("active");
    let mut receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind: "asp".to_owned(),
        artifact_mode: "release".to_owned(),
        stable_path: root.path().join("runtime/bin/asp"),
        source_path: source.clone(),
        source_generation: runtime_artifact_source_generation(&source).expect("generation"),
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
        artifact_digest: digest.to_string(),
        identity_kind: "content".to_owned(),
        identity_value: digest.clone(),
        identity_algorithm: "blake3-256".to_owned(),
        source_executable_identity: None,
        active_executable_identity: None,
    };
    assert!(
        admit_runtime_invoker(&source, &receipt, &active)
            .expect_err("missing identity")
            .contains("invoker-artifact-not-active")
    );
    receipt.source_executable_identity =
        Some(executable_identity(&source, &digest).expect("source identity"));
    receipt.active_executable_identity =
        Some(executable_identity(&active, &digest).expect("active identity"));
    std::fs::remove_file(&active).expect("remove active");
    let drift = root
        .path()
        .join("runtime/artifacts/blake3-256")
        .join("a".repeat(64))
        .join("asp");
    std::fs::create_dir_all(drift.parent().expect("drift parent")).expect("mkdir");
    std::fs::copy(&source, &drift).expect("drift");
    assert!(
        admit_runtime_invoker(&source, &receipt, &drift)
            .expect_err("digest drift")
            .contains("active-artifact-digest-drift")
    );
}

#[test]
fn unknown_source_generation_algorithm_is_rejected() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("source");
    std::fs::write(&source, b"runtime").expect("source");
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind: "asp".to_owned(),
        artifact_mode: "dev".to_owned(),
        stable_path: root.path().join("stable"),
        source_path: source.clone(),
        source_generation: "x".to_owned(),
        source_generation_algorithm: "unknown-v1".to_owned(),
        artifact_digest: "a".repeat(64),
        identity_kind: "content".to_owned(),
        identity_value:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"fixture-digest",
            ),
        identity_algorithm: "blake3-256".to_owned(),
        source_executable_identity: None,
        active_executable_identity: None,
    };
    assert!(
        receipt
            .source_generation_matches(&source)
            .expect_err("unknown algorithm")
            .contains("unknown Runtime artifact source generation algorithm")
    );
}
