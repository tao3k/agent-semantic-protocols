//! Production-command coverage for the real Cargo-built ASP executable.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct IsolatedStateHome(PathBuf);

impl IsolatedStateHome {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-install-binary-production-command-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create isolated ASP State Home");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for IsolatedStateHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn built_asp_install_binary_publishes_typed_pending_activation_to_isolated_state_home() {
    let state_home = IsolatedStateHome::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let asp = Path::new(env!("CARGO_BIN_EXE_asp"));
    assert!(
        asp.is_file(),
        "Cargo-built asp binary must exist: {}",
        asp.display()
    );

    let output = Command::new(asp)
        .args(["install", "binary"])
        .current_dir(workspace_root)
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("execute actual Cargo-built asp install binary");
    assert!(
        output.status.success(),
        "actual asp install binary must publish an immutable activation candidate: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let pending_path = agent_semantic_artifacts::runtime_artifact_publication::
        runtime_artifact_activation_event_path(state_home.path());
    let activation = serde_json::from_slice::<
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent,
    >(&std::fs::read(&pending_path).expect("read pending activation receipt"))
    .expect("decode pending activation receipt");
    let expected_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(asp).expect("read Cargo-built asp binary"),
        );
    assert!(activation.activation_generation > 0);
    assert_eq!(activation.artifact_digest, expected_digest);
    assert!(activation.artifact_path.is_file());
    assert!(activation.artifact_path.components().any(|component| {
        component.as_os_str() == activation.artifact_digest.content_digest().as_str()
    }));
    assert!(
        !state_home
            .path()
            .join("runtime/activation/applied.json")
            .exists(),
        "install cannot claim applied authority before the Runtime actor commits"
    );
    assert!(
        !state_home
            .path()
            .join("runtime/server/endpoint.v1.json")
            .exists(),
        "install cannot publish a Runtime endpoint"
    );
    assert!(
        !state_home.path().join("runtime/bin/asp").exists(),
        "install cannot switch a mutable serving alias before activation commit"
    );
    assert!(
        !state_home
            .path()
            .join("runtime/leases/artifact-publication.v1.json")
            .exists(),
        "successful production command consumes its publication lease exactly once"
    );
}
