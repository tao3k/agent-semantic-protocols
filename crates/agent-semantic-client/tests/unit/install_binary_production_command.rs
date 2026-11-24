// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Production-command coverage for the real Cargo-built ASP executable.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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

#[tokio::test]
async fn built_asp_install_binary_publishes_immutable_activation_candidate() {
    let _install_guard = crate::install_binary_test_guard::acquire();
    let state_home = IsolatedStateHome::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let built_asp = Path::new(env!("CARGO_BIN_EXE_asp"));
    assert!(
        built_asp.is_file(),
        "Cargo-built asp binary must exist: {}",
        built_asp.display()
    );
    let build_fixture = tempfile::tempdir().expect("isolated binary build directory");
    let asp = build_fixture.path().join("asp");
    std::fs::copy(built_asp, &asp).expect("copy ASP binary fixture");
    #[cfg(feature = "mrr-data-search-composition")]
    let mrr_owner = {
        let owner_name = format!("mrr-native-worker{}", std::env::consts::EXE_SUFFIX);
        let source = built_asp.parent().unwrap().join(&owner_name);
        let candidate = build_fixture.path().join(owner_name);
        std::fs::copy(source, &candidate).expect("copy declared MRR search execution asset");
        candidate
    };
    let isolated_home = state_home.path().join("home");
    let output = Command::new(&asp)
        .args(["install", "binary"])
        .current_dir(workspace_root)
        .env("ASP_STATE_HOME", state_home.path())
        .env("HOME", &isolated_home)
        .output()
        .expect("execute actual Cargo-built asp install binary");
    assert!(
        output.status.success(),
        "actual asp install binary must publish an immutable activation candidate: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let install_stdout = String::from_utf8(output.stdout).expect("install receipt UTF-8");
    assert!(install_stdout.contains("runtimeServerLifecycle=resident-owner-independent"));
    assert!(!state_home.path().join("hooks/current").exists());
    #[cfg(feature = "mrr-data-search-composition")]
    {
        let installed_owner = state_home
            .path()
            .join("runtime/artifacts/active/mrr-search");
        assert_eq!(
            std::fs::read(installed_owner).expect("published built-in search execution asset"),
            std::fs::read(mrr_owner).expect("original declared MRR asset"),
            "binary publication preserves the exact search owner bytes"
        );
    }

    let pending_path = agent_semantic_artifacts::runtime_artifact_activation::
        runtime_artifact_activation_event_path(state_home.path());
    let activation = serde_json::from_slice::<
        agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    >(&std::fs::read(&pending_path).expect("read pending activation receipt"))
    .expect("decode pending activation receipt");
    let expected_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&asp).expect("read Cargo-built asp binary"),
        );
    assert!(!activation.publication_nonce.is_empty());
    assert_eq!(activation.artifact_digest, expected_digest);
    assert!(activation.artifact_path.is_file());
    assert!(activation.artifact_path.components().any(|component| {
        component.as_os_str() == activation.bundle_digest.content_digest().as_str()
    }));
    assert!(
        !state_home
            .path()
            .join("runtime/artifacts/activation/applied.json")
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
    assert_eq!(
        std::fs::canonicalize(state_home.path().join("runtime/bin/asp"))
            .expect("canonical client launcher"),
        std::fs::canonicalize(&activation.artifact_path)
            .expect("canonical pending activation artifact"),
        "install must switch only the client launcher to the pending immutable candidate"
    );
    let path_visible_asp = isolated_home.join(".local/bin/asp");
    assert_eq!(
        std::fs::read_link(state_home.path().join("runtime/bin/asp"))
            .expect("read Runtime compatibility alias"),
        path_visible_asp,
        "Runtime compatibility alias must point to the PATH-visible install"
    );
    assert_ne!(
        std::fs::read_link(&path_visible_asp).expect("read PATH-visible ASP entry"),
        state_home.path().join("runtime/bin/asp"),
        "PATH-visible ASP entry must not point back to the Runtime alias"
    );
    assert!(
        state_home.path().join("runtime/artifacts/active").exists(),
        "install must switch the sole active generation selector"
    );
    assert!(
        !state_home
            .path()
            .join("runtime/artifacts/leases/artifact-publication.v1.json")
            .exists(),
        "successful production command consumes its publication lease exactly once"
    );
}
