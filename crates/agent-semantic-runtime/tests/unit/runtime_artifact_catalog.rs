//! Runtime artifact catalog tests.

use std::path::{Path, PathBuf};

use agent_semantic_config::runtime_dev::{ArtifactOrigin, RuntimeArtifactMode};
use agent_semantic_runtime::runtime_artifact_catalog::{
    RuntimeArtifactCatalog, RuntimeArtifactReceipt, load_runtime_artifact_catalog,
};

#[tokio::test]
async fn tokio_loader_constructs_one_dev_catalog_generation() {
    let state_home = tempfile::tempdir().expect("state home");
    let checkout = tempfile::tempdir().expect("checkout");
    tokio::fs::write(
        state_home.path().join("asp.toml"),
        format!("[dev]\nenabled = true\nroot = {:?}\n", checkout.path()),
    )
    .await
    .expect("dev config");
    let catalog = load_runtime_artifact_catalog(state_home.path())
        .await
        .expect("runtime catalog");
    assert_eq!(
        catalog.mode(),
        &RuntimeArtifactMode::Dev {
            root: checkout.path().canonicalize().expect("canonical checkout")
        }
    );
    assert_eq!(catalog.mode_label(), "dev");
    assert!(catalog.digest().starts_with("blake3-256:"));
    tokio::fs::remove_file(state_home.path().join("asp.toml"))
        .await
        .expect("remove config after load");

    let receipt = RuntimeArtifactReceipt {
        origin: ArtifactOrigin::DevelopWorkspace,
        checkout_root: Some(checkout.path().canonicalize().expect("canonical checkout")),
        reference: artifact_reference(
            ArtifactOrigin::DevelopWorkspace,
            checkout.path().join("target/debug/asp"),
            Some(checkout.path().canonicalize().expect("canonical checkout")),
        ),
    };
    assert!(catalog.admits(&receipt), "admission uses resident memory");
}

#[tokio::test]
async fn enabled_dev_root_must_exist_when_the_daemon_catalog_is_loaded() {
    let state_home = tempfile::tempdir().expect("state home");
    let missing = state_home.path().join("missing-checkout");
    tokio::fs::write(
        state_home.path().join("asp.toml"),
        format!("[dev]\nenabled = true\nroot = {:?}\n", missing),
    )
    .await
    .expect("dev config");

    let error = load_runtime_artifact_catalog(state_home.path())
        .await
        .expect_err("daemon must fail closed on a missing development checkout");
    assert!(error.contains("canonicalize runtime [dev].root"));
}

#[tokio::test]
async fn missing_config_selects_release_without_path_fallback() {
    let state_home = tempfile::tempdir().expect("state home");
    let catalog = load_runtime_artifact_catalog(state_home.path())
        .await
        .expect("release catalog");
    assert_eq!(catalog.mode(), &RuntimeArtifactMode::Release);
    assert_eq!(catalog.mode_label(), "release");
    assert!(catalog.digest().starts_with("blake3-256:"));
    assert!(catalog.admits(&RuntimeArtifactReceipt {
        origin: ArtifactOrigin::LockedRelease,
        checkout_root: None,
        reference: artifact_reference(
            ArtifactOrigin::LockedRelease,
            state_home.path().join("runtime/profiles/asp/bin/asp"),
            None,
        ),
    }));
}

fn artifact_reference(
    origin: ArtifactOrigin,
    executable_path: PathBuf,
    checkout_root: Option<PathBuf>,
) -> agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactReference {
    agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactReference::new(
        "asp",
        origin,
        executable_path,
        format!("blake3-256:{}", "0".repeat(64)),
        checkout_root,
    )
}

#[cfg(unix)]
#[tokio::test]
async fn release_retention_keeps_only_generations_reachable_from_runtime_slots() {
    let temporary = tempfile::tempdir().expect("temporary runtime state");
    let runtime_root = temporary.path().join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let algorithm_root = artifact_root.join("blake3-256");
    let asp_digest = "1".repeat(64);
    let provider_digest = "2".repeat(64);
    let stale_digest = "3".repeat(64);
    let asp_artifact = algorithm_root.join(&asp_digest).join("asp");
    let provider_artifact = algorithm_root.join(&provider_digest).join("asp-rust");
    let stale_artifact = algorithm_root.join(&stale_digest).join("asp-old");
    for artifact in [&asp_artifact, &provider_artifact, &stale_artifact] {
        std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
            .expect("create artifact generation");
        std::fs::write(artifact, b"runtime artifact").expect("write artifact");
    }
    let asp_slot = runtime_root.join("bin/asp");
    let provider_slot = runtime_root.join("profiles/rust/asp-rust");
    std::fs::create_dir_all(asp_slot.parent().expect("asp slot parent"))
        .expect("create runtime bin");
    std::fs::create_dir_all(provider_slot.parent().expect("provider slot parent"))
        .expect("create runtime profiles");
    std::os::unix::fs::symlink(&asp_artifact, &asp_slot).expect("publish asp slot");
    std::os::unix::fs::symlink(&provider_artifact, &provider_slot).expect("publish provider slot");

    let receipt =
        agent_semantic_runtime::runtime_artifact_retention::prune_unreachable_runtime_artifacts(
            &artifact_root,
        )
        .await
        .expect("prune unreachable generations");

    assert_eq!(receipt.scanned_generation_count, 3);
    assert_eq!(receipt.retained_generation_count, 2);
    assert_eq!(receipt.removed_generation_count, 1);
    assert_eq!(receipt.rollback_generations_per_binary, 0);
    assert_eq!(receipt.protected_digests, vec![asp_digest, provider_digest]);
    assert!(!algorithm_root.join(stale_digest).exists());
}

#[cfg(unix)]
#[tokio::test]
async fn developer_publication_rejects_a_source_outside_the_configured_checkout() {
    let temporary = tempfile::tempdir().expect("temporary developer state");
    let state_home = temporary.path().join("state");
    let checkout = temporary.path().join("checkout");
    let source = temporary.path().join("foreign/asp");
    let runtime_root = state_home.join("runtime");
    let target = runtime_root.join("bin/asp");
    let artifact_root = runtime_root.join("artifacts");
    std::fs::create_dir_all(&checkout).expect("create developer checkout");
    std::fs::create_dir_all(source.parent().expect("source parent"))
        .expect("create foreign build directory");
    std::fs::create_dir_all(&state_home).expect("create state home");
    std::fs::write(&source, b"foreign-runtime-artifact").expect("write foreign artifact");
    std::fs::write(
        state_home.join("asp.toml"),
        format!(
            "[dev]\nenabled = true\nroot = {:?}\n",
            checkout.display().to_string()
        ),
    )
    .expect("write runtime configuration");

    let error = agent_semantic_runtime::runtime_artifact_catalog::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        &artifact_root,
        "asp",
    )
    .await
    .expect_err("foreign source must fail closed");

    assert!(error.contains("escapes configured root"), "{error}");
    assert!(!artifact_root.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn developer_publication_is_tokio_owned_and_creates_no_digest_history() {
    let temporary = tempfile::tempdir().expect("temporary developer state");
    let state_home = temporary.path().join("state");
    let checkout = temporary.path().join("checkout");
    let source = checkout.join("target/debug/asp");
    let runtime_root = state_home.join("runtime");
    let target = runtime_root.join("bin/asp");
    let artifact_root = runtime_root.join("artifacts");
    let legacy_artifact = artifact_root
        .join("blake3-256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/asp");
    std::fs::create_dir_all(source.parent().expect("source parent"))
        .expect("create developer build directory");
    std::fs::create_dir_all(&state_home).expect("create state home");
    std::fs::write(&source, b"developer-v1").expect("write developer artifact");
    std::fs::create_dir_all(legacy_artifact.parent().expect("legacy artifact parent"))
        .expect("create legacy artifact generation");
    std::fs::write(&legacy_artifact, b"legacy-digest-copy")
        .expect("write legacy artifact generation");
    std::fs::write(
        state_home.join("asp.toml"),
        format!(
            "[dev]\nenabled = true\nroot = {:?}\n",
            checkout.display().to_string()
        ),
    )
    .expect("write runtime configuration");

    let publication = agent_semantic_runtime::runtime_artifact_catalog::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        &artifact_root,
        "asp",
    )
    .await
    .expect("publish developer artifact");

    assert_eq!(publication.path, target);
    assert_eq!(
        std::fs::canonicalize(&publication.path).unwrap(),
        std::fs::canonicalize(&source).unwrap()
    );
    assert!(!artifact_root.exists());
    std::fs::write(&source, b"developer-v2").expect("replace developer artifact");
    assert_eq!(std::fs::read(&publication.path).unwrap(), b"developer-v2");
}

#[cfg(unix)]
#[tokio::test]
async fn release_publication_atomically_advances_one_slot_and_reclaims_the_old_generation() {
    let temporary = tempfile::tempdir().expect("temporary release state");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("build/asp");
    let runtime_root = state_home.join("runtime");
    let target = runtime_root.join("bin/asp");
    let artifact_root = runtime_root.join("artifacts");
    std::fs::create_dir_all(source.parent().expect("source parent"))
        .expect("create release build directory");
    std::fs::write(&source, b"release-v1").expect("write first release artifact");

    let first = agent_semantic_runtime::runtime_artifact_catalog::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        &artifact_root,
        "asp",
    )
    .await
    .expect("publish first release artifact");
    std::fs::write(&source, b"release-v2").expect("write second release artifact");
    let second = agent_semantic_runtime::runtime_artifact_catalog::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        &artifact_root,
        "asp",
    )
    .await
    .expect("publish second release artifact");

    assert_ne!(first.artifact_digest, second.artifact_digest);
    assert_eq!(std::fs::read(&target).unwrap(), b"release-v2");
    assert!(
        !artifact_root
            .join("blake3-256")
            .join(first.artifact_digest)
            .exists()
    );
    assert!(
        artifact_root
            .join("blake3-256")
            .join(second.artifact_digest)
            .exists()
    );
}

#[test]
fn developer_reference_uses_verified_build_output_without_artifact_staging() {
    let root = PathBuf::from("/checkout/agent-semantic-protocols");
    let catalog = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Dev { root: root.clone() });
    let reference = artifact_reference(
        ArtifactOrigin::DevelopWorkspace,
        root.join("target/debug/asp"),
        Some(root.clone()),
    );

    catalog
        .admit_reference(&reference, Path::new("/runtime"))
        .expect("developer build output is the execution authority");
    assert!(!reference.executable_path.starts_with("/runtime/artifacts"));
}

#[test]
fn release_reference_requires_a_stable_runtime_slot() {
    let catalog = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release);
    let runtime_root = Path::new("/runtime");
    let stable_binary = artifact_reference(
        ArtifactOrigin::LockedRelease,
        runtime_root.join("bin/asp"),
        None,
    );
    catalog
        .admit_reference(&stable_binary, runtime_root)
        .expect("stable release binary slot");

    let stable_profile = artifact_reference(
        ArtifactOrigin::LockedRelease,
        runtime_root.join("profiles/asp/bin/asp"),
        None,
    );
    catalog
        .admit_reference(&stable_profile, runtime_root)
        .expect("stable release profile slot");

    let digest_lattice = artifact_reference(
        ArtifactOrigin::LockedRelease,
        runtime_root.join("artifacts/blake3-256/deadbeef/asp"),
        None,
    );
    assert!(
        catalog
            .admit_reference(&digest_lattice, runtime_root)
            .is_err()
    );
}

#[test]
fn receipt_rejects_reference_identity_drift() {
    let root = PathBuf::from("/checkout/agent-semantic-protocols");
    let catalog = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Dev { root: root.clone() });
    let receipt = RuntimeArtifactReceipt {
        origin: ArtifactOrigin::DevelopWorkspace,
        checkout_root: Some(root.clone()),
        reference: artifact_reference(
            ArtifactOrigin::LockedRelease,
            root.join("target/debug/asp"),
            None,
        ),
    };
    assert!(!catalog.admits(&receipt));
}

#[test]
fn dev_catalog_rejects_release_and_foreign_checkout_receipts() {
    let catalog = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Dev {
        root: PathBuf::from("/checkout/agent-semantic-protocols"),
    });
    for receipt in [
        RuntimeArtifactReceipt {
            origin: ArtifactOrigin::LockedRelease,
            checkout_root: None,
            reference: artifact_reference(
                ArtifactOrigin::LockedRelease,
                PathBuf::from("/runtime/profiles/asp/bin/asp"),
                None,
            ),
        },
        RuntimeArtifactReceipt {
            origin: ArtifactOrigin::DevelopWorkspace,
            checkout_root: Some(Path::new("/checkout/other").to_path_buf()),
            reference: artifact_reference(
                ArtifactOrigin::DevelopWorkspace,
                PathBuf::from("/checkout/other/target/debug/asp"),
                Some(PathBuf::from("/checkout/other")),
            ),
        },
    ] {
        assert!(!catalog.admits(&receipt));
    }
}

#[test]
fn catalog_identity_binds_mode_and_canonical_checkout() {
    let first = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Dev {
        root: PathBuf::from("/checkout/first"),
    });
    let second = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Dev {
        root: PathBuf::from("/checkout/second"),
    });
    let release = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release);

    assert_ne!(first.digest(), second.digest());
    assert_ne!(first.digest(), release.digest());
    assert_ne!(second.digest(), release.digest());
}

#[test]
fn catalog_identity_binds_provider_catalog_generation() {
    let first = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release)
        .with_provider_catalog_generation("blake3-256:first");
    let second = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release)
        .with_provider_catalog_generation("blake3-256:second");

    assert_ne!(first.digest(), second.digest());
    assert_ne!(
        first.digest(),
        RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release).digest()
    );
}

#[tokio::test]
async fn tokio_loader_admits_provider_catalog_generation_into_daemon_identity() {
    let state_home = tempfile::tempdir().expect("state home");
    tokio::fs::create_dir_all(state_home.path().join("runtime"))
        .await
        .expect("runtime directory");
    tokio::fs::write(
        state_home.path().join("runtime/provider-catalog.v1.json"),
        r#"{"catalogGeneration":"blake3-256:provider-generation"}"#,
    )
    .await
    .expect("provider catalog");

    let loaded = load_runtime_artifact_catalog(state_home.path())
        .await
        .expect("runtime catalog with provider generation");
    let expected = RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release)
        .with_provider_catalog_generation("blake3-256:provider-generation");
    assert_eq!(loaded.digest(), expected.digest());
}
