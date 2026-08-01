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
    }));
    assert!(!catalog.admits(&RuntimeArtifactReceipt {
        origin: ArtifactOrigin::PathFallback,
        checkout_root: None,
    }));
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
        },
        RuntimeArtifactReceipt {
            origin: ArtifactOrigin::DevelopWorkspace,
            checkout_root: Some(Path::new("/checkout/other").to_path_buf()),
        },
        RuntimeArtifactReceipt {
            origin: ArtifactOrigin::PathFallback,
            checkout_root: None,
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
