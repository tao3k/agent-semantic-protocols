// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::collections::BTreeSet;

use super::publish_runtime_artifact;
use crate::runtime_artifact_retention::RuntimeArtifactCandidatePreparationLease;
use crate::runtime_artifact_retention::prune_unreachable_runtime_artifacts;

#[tokio::test]
async fn canonical_publication_retains_only_active_and_healthy_artifact_digests() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let target = state_home.join("runtime/bin/asp");

    let first_source = state_home.join("asp-first");
    let second_source = state_home.join("asp-second");
    let third_source = state_home.join("asp-third");
    std::fs::write(&first_source, b"first-runtime").expect("write first Runtime candidate");
    std::fs::write(&second_source, b"second-runtime").expect("write second Runtime candidate");
    std::fs::write(&third_source, b"third-runtime").expect("write third Runtime candidate");

    let first = publish_runtime_artifact(state_home, &first_source, &target, "test")
        .await
        .expect("publish first Runtime candidate");
    let second = publish_runtime_artifact(state_home, &second_source, &target, "test")
        .await
        .expect("publish second Runtime candidate");
    let third = publish_runtime_artifact(state_home, &third_source, &target, "test")
        .await
        .expect("publish third Runtime candidate");

    let artifact_root = state_home.join("runtime/artifacts/generations");
    let retained = std::fs::read_dir(&artifact_root)
        .expect("read immutable Runtime artifact store")
        .map(|entry| {
            entry
                .expect("read immutable Runtime artifact entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(retained.len(), 2);
    assert!(!state_home.join("runtime/artifacts/blake3-256").exists());
    assert!(!state_home.join("runtime/artifacts/bundles").exists());
    assert_ne!(first.bundle_digest, second.bundle_digest);
    assert_ne!(second.bundle_digest, third.bundle_digest);
    assert!(retained.contains(first.bundle_digest.content_digest().as_str()));
    assert!(retained.contains(third.bundle_digest.content_digest().as_str()));
}

#[tokio::test]
async fn canonical_publication_retires_superseded_physical_stores() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let artifact_root = state_home.join("runtime/artifacts");
    let old_member_store = artifact_root
        .join("blake3-256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let old_bundle_store = artifact_root
        .join("bundles/asp/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    std::fs::create_dir_all(&old_member_store).expect("create superseded member store");
    std::fs::create_dir_all(&old_bundle_store).expect("create superseded bundle store");
    std::fs::write(old_member_store.join("asp"), b"superseded-member")
        .expect("write superseded member");
    std::fs::write(old_bundle_store.join("bundle.json"), b"superseded-bundle")
        .expect("write superseded bundle");
    let old_provider_root = state_home.join("runtime/providers");
    std::fs::create_dir_all(old_provider_root.join("receipts"))
        .expect("create superseded Provider staging root");
    std::fs::write(
        old_provider_root.join("receipts/rust.lock.toml"),
        b"superseded",
    )
    .expect("write superseded Provider receipt");

    let source = state_home.join("asp-current");
    let target = state_home.join("bin/asp");
    std::fs::write(&source, b"current-runtime").expect("write current Runtime candidate");
    publish_runtime_artifact(state_home, &source, &target, "test")
        .await
        .expect("publish canonical Runtime generation");

    assert!(!artifact_root.join("blake3-256").exists());
    assert!(!artifact_root.join("bundles").exists());
    assert!(!old_provider_root.exists());
    assert!(artifact_root.join("generations").is_dir());
    assert!(artifact_root.join("active").exists());
}

#[tokio::test]
async fn retention_does_not_retire_superseded_stores_before_a_canonical_selection() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let artifact_root = temp.path().join("runtime/artifacts");
    let old_member_store = artifact_root.join("blake3-256");
    std::fs::create_dir_all(&old_member_store).expect("create superseded member store");
    std::fs::write(old_member_store.join("unselected"), b"not-yet-migrated")
        .expect("write unselected member");

    let receipt = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("retention without a canonical selection");

    assert_eq!(receipt.retired_superseded_store_count, 0);
    assert!(old_member_store.is_dir());
}

#[cfg(unix)]
#[tokio::test]
async fn retention_rejects_a_superseded_store_symlink_without_following_it() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let artifact_root = state_home.join("runtime/artifacts");
    let source = state_home.join("asp-current");
    let target = state_home.join("bin/asp");
    std::fs::write(&source, b"current-runtime").expect("write current Runtime candidate");
    publish_runtime_artifact(state_home, &source, &target, "test")
        .await
        .expect("publish canonical Runtime generation");

    let outside = state_home.join("outside-store");
    std::fs::create_dir_all(&outside).expect("create outside store");
    std::fs::write(outside.join("preserved"), b"outside").expect("write outside marker");
    std::os::unix::fs::symlink(&outside, artifact_root.join("blake3-256"))
        .expect("create superseded-store symlink");

    let error = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect_err("symlinked superseded store must fail closed");
    assert!(error.contains("reasonKind=superseded-artifact-store-type-conflict"));
    assert_eq!(
        std::fs::read(outside.join("preserved")).unwrap(),
        b"outside"
    );
    assert!(artifact_root.join("blake3-256").is_symlink());
}

#[tokio::test]
async fn retention_retires_binary_namespaced_candidate_leases() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let artifact_root = state_home.join("runtime/artifacts");
    let source = state_home.join("asp-current");
    let target = state_home.join("bin/asp");
    std::fs::write(&source, b"current-runtime").expect("write current Runtime candidate");
    publish_runtime_artifact(state_home, &source, &target, "test")
        .await
        .expect("publish canonical Runtime generation");

    let old_lease_root = artifact_root.join("leases/candidates/asp");
    std::fs::create_dir_all(&old_lease_root).expect("create binary-namespaced lease root");
    std::fs::write(
        old_lease_root
            .join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.lock"),
        b"superseded-lease",
    )
    .expect("write superseded lease");

    let receipt = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("retire binary-namespaced lease root");

    assert_eq!(receipt.retired_superseded_lease_directory_count, 1);
    assert!(!old_lease_root.exists());
    assert!(!artifact_root.join("retired").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn canonical_publication_retires_only_the_misplaced_state_home_launcher_root() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let misplaced_root = state_home.join("bin");
    std::fs::create_dir_all(&misplaced_root).expect("create misplaced launcher root");
    std::os::unix::fs::symlink(
        state_home.join("runtime/artifacts/active/asp"),
        misplaced_root.join("asp"),
    )
    .expect("create misplaced managed launcher");

    let source = state_home.join("asp-current");
    let target = state_home.join("runtime/bin/asp");
    std::fs::write(&source, b"current-runtime").expect("write current Runtime candidate");
    publish_runtime_artifact(state_home, &source, &target, "test")
        .await
        .expect("publish canonical Runtime generation");

    assert!(!misplaced_root.exists());
    assert!(target.exists());
    let receipt: crate::runtime_artifact_retention::RuntimeArtifactRetentionReceipt =
        serde_json::from_slice(
            &std::fs::read(state_home.join("runtime/artifacts/retention-receipt.json"))
                .expect("read retention receipt"),
        )
        .expect("decode retention receipt");
    assert_eq!(receipt.retired_misplaced_launcher_root_count, 1);
}

#[tokio::test]
async fn retention_preserves_a_state_home_bin_with_unmanaged_content() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let source = state_home.join("asp-current");
    let target = state_home.join("runtime/bin/asp");
    std::fs::write(&source, b"current-runtime").expect("write current Runtime candidate");
    publish_runtime_artifact(state_home, &source, &target, "test")
        .await
        .expect("publish canonical Runtime generation");

    let unmanaged = state_home.join("bin/user-tool");
    std::fs::create_dir_all(unmanaged.parent().unwrap()).expect("create State Home bin");
    std::fs::write(&unmanaged, b"user-owned").expect("write unmanaged launcher content");

    let error = prune_unreachable_runtime_artifacts(&state_home.join("runtime/artifacts"))
        .await
        .expect_err("unmanaged State Home bin content must fail closed");
    assert!(error.contains("reasonKind=misplaced-launcher-entry-not-managed"));
    assert_eq!(std::fs::read(&unmanaged).unwrap(), b"user-owned");
}

#[tokio::test]
async fn live_preparation_lease_preserves_candidate_and_dead_owner_is_reclaimed() {
    let temp = tempfile::tempdir().expect("create temporary State Home");
    let state_home = temp.path();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let candidate = state_home
        .join("runtime/artifacts/generations")
        .join(digest);
    let lease = RuntimeArtifactCandidatePreparationLease::acquire(state_home, "asp", digest)
        .expect("acquire candidate preparation lease");
    std::fs::create_dir_all(&candidate).expect("create in-flight candidate");
    std::fs::write(candidate.join("bundle.json"), b"in-flight")
        .expect("write in-flight candidate marker");

    let artifact_root = state_home.join("runtime/artifacts");
    let live = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("retain candidate owned by live publisher");
    assert_eq!(live.scanned_generation_count, 1);
    assert_eq!(live.removed_generation_count, 0);
    assert!(candidate.is_dir());

    drop(lease);
    let dead = prune_unreachable_runtime_artifacts(&artifact_root)
        .await
        .expect("reclaim candidate after producer lease release");
    assert_eq!(dead.scanned_generation_count, 1);
    assert_eq!(dead.removed_generation_count, 1);
    assert!(!candidate.exists());
    assert!(
        !state_home
            .join("runtime/artifacts/leases/candidates")
            .join(format!("{digest}.lock"))
            .exists()
    );
}
