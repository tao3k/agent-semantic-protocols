// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_artifacts::RuntimeArtifactStateLayout;
use agent_semantic_artifacts::runtime_artifact_bundle_digest_from_member_path;
use agent_semantic_artifacts::runtime_artifact_member_is_digest_addressed;

#[test]
fn mutable_runtime_artifact_state_has_one_physical_authority() {
    let state_home = Path::new("/tmp/asp-state-layout-contract");
    let layout = RuntimeArtifactStateLayout::new(state_home);

    assert_eq!(layout.root(), state_home.join("runtime/artifacts"));
    assert_eq!(layout.generation_store(), layout.root().join("generations"));
    assert_eq!(layout.active_slot(), layout.root().join("active"));
    assert_eq!(layout.healthy_slot(), layout.root().join("healthy"));
    assert_eq!(layout.activation(), layout.root().join("activation"));
    assert_eq!(layout.leases(), layout.root().join("leases"));

    for path in [
        layout.generation_store(),
        layout.active_slot(),
        layout.healthy_slot(),
        layout.activation(),
        layout.leases(),
    ] {
        assert!(path.starts_with(layout.root()));
    }
}

#[test]
fn canonical_generation_member_projects_bundle_identity() {
    let digest = "a".repeat(64);
    let path = format!("/state/runtime/artifacts/generations/{digest}/asp");
    assert_eq!(
        runtime_artifact_bundle_digest_from_member_path(Path::new(&path)),
        Some(digest)
    );
}

#[test]
fn mutable_or_malformed_paths_are_not_bundle_identities() {
    assert_eq!(
        runtime_artifact_bundle_digest_from_member_path(Path::new(
            "/state/runtime/artifacts/active/asp"
        )),
        None
    );
    assert_eq!(
        runtime_artifact_bundle_digest_from_member_path(Path::new(
            "/state/runtime/artifacts/generations/not-a-digest/asp"
        )),
        None
    );
}

#[test]
fn canonical_member_requires_the_generation_store_and_bundle_digest() {
    let temporary = tempfile::tempdir().expect("temporary Runtime artifact root");
    let artifact_root = temporary.path().join("runtime/artifacts");
    let digest = "a".repeat(64);
    let generation = artifact_root.join("generations").join(digest);
    std::fs::create_dir_all(&generation).expect("generation directory");
    let member = generation.join("asp");
    std::fs::write(&member, b"runtime member").expect("generation member");

    assert!(
        runtime_artifact_member_is_digest_addressed(
            &std::fs::canonicalize(&member).expect("canonical member"),
            &artifact_root,
        )
        .expect("validate generation member")
    );
    assert!(
        !runtime_artifact_member_is_digest_addressed(
            &std::fs::canonicalize(&member).expect("canonical member"),
            &temporary.path().join("missing-artifact-root"),
        )
        .expect("missing artifact root is not an identity")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_publication_uses_only_the_canonical_artifact_authority() {
    let temporary = tempfile::tempdir().expect("temporary state home");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp-candidate");
    let target = state_home.join("runtime/bin/asp");
    std::fs::write(&source, b"runtime-candidate-v1").expect("candidate bytes");

    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        "test",
    )
    .await
    .expect("publish Runtime artifact");

    let layout = RuntimeArtifactStateLayout::new(&state_home);
    assert!(layout.generation_store().is_dir());
    assert!(!layout.root().join("blake3-256").exists());
    assert!(!layout.root().join("bundles").exists());
    assert!(layout.active_slot().exists());
    let active_bundle = std::fs::read_link(layout.active_slot()).expect("active bundle selector");
    assert!(
        active_bundle.join("asp").exists(),
        "active bundle member must survive retention"
    );
    assert!(layout.pending_activation().is_file());
    assert!(layout.leases().is_dir());
    assert!(
        !layout.publication_lease().exists(),
        "successful publication must consume its quiescence lease"
    );
}
