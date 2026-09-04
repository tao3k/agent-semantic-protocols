use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

#[tokio::test]
async fn built_asp_install_canonicalizes_pending_identity_and_publishes_a_new_generation() {
    let _install_guard = crate::install_binary_test_guard::acquire();
    let state_home = tempfile::tempdir().expect("isolated State Home");
    let first = install(state_home.path());
    assert_success(&first, "seed isolated production state");

    let activation = agent_semantic_artifacts::runtime_artifact_activation::
        runtime_artifact_activation_event_path(state_home.path());
    let mut event: serde_json::Value =
        serde_json::from_slice(&fs::read(&activation).expect("seed activation receipt"))
            .expect("decode activation receipt");
    let canonical = event["artifactDigest"]
        .as_str()
        .expect("typed artifact digest")
        .to_owned();
    let raw = canonical
        .strip_prefix("blake3-256:")
        .expect("canonical digest")
        .to_owned();
    event["artifactDigest"] = serde_json::Value::String(raw.clone());
    if !event["previousArtifactDigest"].is_null() {
        event["previousArtifactDigest"] = serde_json::json!({
            "value": raw,
            "algorithm": "blake3-256"
        });
    }
    fs::write(&activation, serde_json::to_vec_pretty(&event).unwrap())
        .expect("seed raw activation identity");
    let migrated_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        state_home.path(),
    )
    .await
    .expect("migrate raw activation identity")
    .expect("pending activation event");
    assert!(
        migrated_event
            .artifact_digest
            .as_str()
            .starts_with("blake3-256:")
    );

    let migrated = install(state_home.path());
    assert_success(&migrated, "migrate raw existing state");
    let committed: serde_json::Value =
        serde_json::from_slice(&fs::read(&activation).expect("newer pending activation receipt"))
            .expect("decode newer pending activation receipt");
    assert!(
        committed["artifactDigest"]
            .as_str()
            .is_some_and(|value| value.starts_with("blake3-256:"))
    );
    assert_ne!(
        committed["publicationNonce"].as_str(),
        Some(migrated_event.publication_nonce.as_str())
    );
    assert_eq!(
        committed["artifactPath"].as_str(),
        Some(migrated_event.artifact_path.to_string_lossy().as_ref()),
        "same content must reuse one immutable artifact while publishing a distinct publication"
    );
    assert!(!state_home.path().join("runtime/resident/active").exists());
    assert!(!state_home.path().join("runtime/resident/healthy").exists());
    assert!(
        !state_home
            .path()
            .join("runtime/activation/applied.json")
            .exists()
    );

    let mut malformed = committed;
    malformed["artifactDigest"] = serde_json::Value::String("NOT-A-DIGEST".to_owned());
    fs::write(&activation, serde_json::to_vec_pretty(&malformed).unwrap())
        .expect("seed malformed activation identity");
    let error = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        state_home.path(),
    )
    .await
    .expect_err("malformed identity must fail closed");
    assert!(error.contains("reasonKind=artifact-identity-incomplete"));
    assert!(migrated_event.artifact_path.is_file());
    assert!(
        !state_home
            .path()
            .join("runtime/activation/applied.json")
            .exists()
    );
}

fn install(state_home: &Path) -> Output {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "binary"])
        .current_dir(workspace)
        .env("ASP_STATE_HOME", state_home)
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("run actual built asp install binary")
}

fn assert_success(output: &Output, phase: &str) {
    assert!(
        output.status.success(),
        "{phase}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
