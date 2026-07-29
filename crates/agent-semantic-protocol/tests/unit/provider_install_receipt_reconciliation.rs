use std::time::{SystemTime, UNIX_EPOCH};

use super::provider_install_receipt_matches_artifact;
use super::{read_provider_install_receipt, reconcile_provider_install_receipt_in_lock_dir};

#[test]
fn provider_receipt_is_resigned_after_runtime_binary_becomes_a_cas_link() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-receipt-reconciliation-{}-{nonce}",
        std::process::id()
    ));
    let runtime_binary = root.join("runtime/bin/rs-harness");
    let immutable_binary = root.join("runtime/artifacts/blake3-256/digest/rs-harness");
    let lock_dir = root.join("runtime/providers/receipts");
    let lock_path = lock_dir.join("rust.lock.toml");
    std::fs::create_dir_all(runtime_binary.parent().expect("runtime parent"))
        .expect("create runtime bin");
    std::fs::create_dir_all(immutable_binary.parent().expect("artifact parent"))
        .expect("create artifact");
    std::fs::create_dir_all(&lock_dir).expect("create lock dir");
    std::fs::write(&runtime_binary, b"rs-harness-v1").expect("write runtime binary");
    std::fs::write(
        &lock_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"rust\"\nprovider = \"rs-harness\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"old-content\"\ninstalledEntrypointMetadataDigest = \"old-metadata\"\n",
            runtime_binary.display()
        ),
    )
    .expect("write provider lock");

    std::fs::copy(&runtime_binary, &immutable_binary).expect("publish immutable artifact");
    std::fs::remove_file(&runtime_binary).expect("remove direct runtime binary");
    std::os::unix::fs::symlink(&immutable_binary, &runtime_binary).expect("link runtime binary");

    assert!(
        reconcile_provider_install_receipt_in_lock_dir("rust", &lock_dir, false)
            .expect("reconcile migrated provider receipt")
    );
    assert!(
        !reconcile_provider_install_receipt_in_lock_dir("rust", &lock_dir, false)
            .expect("reconcile current provider receipt")
    );

    let lock: toml::Value =
        toml::from_str(&std::fs::read_to_string(&lock_path).expect("read provider lock"))
            .expect("parse provider lock");
    assert_eq!(
        lock["installedEntrypointDigest"].as_str(),
        Some(
            agent_semantic_content_identity::file_content_digest_v1(&runtime_binary)
                .expect("content digest")
                .as_str()
        )
    );
    assert_eq!(
        lock["installedEntrypointMetadataDigest"].as_str(),
        Some(
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&runtime_binary)
                .expect("metadata digest")
                .as_str()
        )
    );
    let receipt =
        read_provider_install_receipt("rust", &lock_dir).expect("read typed provider receipt");
    assert_eq!(receipt.language_id, "rust");
    assert_eq!(receipt.provider_id, "rs-harness");
    assert_eq!(receipt.installed_path, runtime_binary);
    assert_eq!(
        receipt.installed_entrypoint_digest,
        lock["installedEntrypointDigest"]
            .as_str()
            .expect("installed content digest")
    );
    assert_eq!(
        receipt.installed_entrypoint_metadata_digest,
        lock["installedEntrypointMetadataDigest"]
            .as_str()
            .expect("installed metadata digest")
    );
    assert_eq!(
        receipt.execution_command_digest,
        lock["executionCommandDigest"]
            .as_str()
            .expect("execution command digest")
    );
    assert!(
        provider_install_receipt_matches_artifact(&receipt, &runtime_binary)
            .expect("compare provider receipt metadata")
    );

    std::fs::remove_file(&runtime_binary).expect("remove runtime link after receipt publication");
    std::fs::remove_file(&immutable_binary)
        .expect("remove immutable binary after receipt publication");
    assert_eq!(
        read_provider_install_receipt("rust", &lock_dir)
            .expect("typed receipt read must not touch provider bytes"),
        receipt
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}
