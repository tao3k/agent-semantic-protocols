use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_bundle_digest;

use super::ASP_PYTHON_GRAPHS_BUNDLE_MEMBER;
use super::VerifiedAspPythonGraphsArtifact;

fn publish_bundle_fixture(state_home: &Path, members: &[(&str, &[u8])]) -> PathBuf {
    let candidate = state_home.join("runtime/artifacts/bundles/asp/test-bundle");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let mut digests = BTreeMap::new();
    for (name, bytes) in members {
        let path = candidate.join(name);
        std::fs::write(&path, bytes).expect("member bytes");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("member executable permission");
        digests.insert((*name).to_owned(), Blake3ContentDigest::from_bytes(bytes));
    }
    let bundle_digest = runtime_artifact_bundle_digest(&digests);
    std::fs::write(
        candidate.join("bundle.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
            "schemaVersion": 1,
            "bundleDigest": bundle_digest,
            "members": digests,
        }))
        .expect("bundle manifest"),
    )
    .expect("write bundle manifest");
    let active = state_home.join("runtime/artifacts/active");
    std::fs::create_dir_all(active.parent().expect("active parent")).expect("resident directory");
    symlink(&candidate, &active).expect("active selector");
    candidate
}

#[tokio::test]
async fn exact_active_bundle_member_is_the_only_graph_worker_authority() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let state_home = temporary.path();
    let candidate = publish_bundle_fixture(
        state_home,
        &[
            ("asp", b"asp"),
            (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
        ],
    );

    let artifact = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(state_home)
        .await
        .expect("verified graph member");

    assert_eq!(
        artifact.executable,
        candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER)
    );
    assert_eq!(
        artifact.content_digest,
        Blake3ContentDigest::from_bytes(b"graphs")
    );
}

#[tokio::test]
async fn missing_graph_member_is_typed_capability_unavailable() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    publish_bundle_fixture(temporary.path(), &[("asp", b"asp")]);

    let error = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
        .await
        .expect_err("missing optional member must fail closed");

    assert!(error.contains("reasonKind=asp-python-graphs-bundle-member-not-installed"));
}

#[tokio::test]
async fn graph_member_digest_drift_fails_before_process_construction() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let candidate = publish_bundle_fixture(
        temporary.path(),
        &[
            ("asp", b"asp"),
            (ASP_PYTHON_GRAPHS_BUNDLE_MEMBER, b"graphs"),
        ],
    );
    std::fs::write(candidate.join(ASP_PYTHON_GRAPHS_BUNDLE_MEMBER), b"drift")
        .expect("corrupt graph member");

    let error = VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(temporary.path())
        .await
        .expect_err("digest drift must fail closed");

    assert!(error.contains("Runtime artifact bundle member digest mismatch"));
}
