use super::reconcile_registered_provider_runtime_binaries_from;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
#[test]
fn registered_scheme_and_python_dangling_runtime_links_are_missing() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-runtime-reconcile-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");
    std::fs::create_dir_all(&artifact_root).expect("temporary artifact root");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    for language_id in ["gerbil-scheme", "python"] {
        let registration = registrations
            .iter()
            .find(|registration| registration.language_id().as_str() == language_id)
            .unwrap_or_else(|| panic!("registered language `{language_id}`"));
        let provider_id = registration.provider_id().as_str();
        assert!(!provider_id.is_empty(), "provider id for `{language_id}`");
        std::os::unix::fs::symlink(
            root.join("missing-provider-artifacts").join(provider_id),
            runtime_bin.join(registration.binary()),
        )
        .unwrap_or_else(|error| panic!("dangling runtime link for `{provider_id}`: {error}"));

        let reconciliation = reconcile_registered_provider_runtime_binaries_from(
            std::slice::from_ref(registration),
            &runtime_bin,
            &artifact_root,
            &provider_lock_dir,
        )
        .unwrap_or_else(|error| panic!("reconcile `{language_id}` / `{provider_id}`: {error}"));
        assert_eq!(reconciliation.registration_count, 1, "{provider_id}");
        assert_eq!(reconciliation.binary_identity_count, 1, "{provider_id}");
        assert_eq!(reconciliation.missing_count, 1, "{provider_id}");

        std::fs::remove_file(runtime_bin.join(registration.binary()))
            .unwrap_or_else(|error| panic!("remove runtime link for `{provider_id}`: {error}"));
    }
    std::fs::remove_dir_all(root).expect("remove temporary runtime bin");
}

#[cfg(unix)]
#[test]
fn canonical_digest_lattice_symlink_is_current_without_rewrite() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-canonical-runtime-entry-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let registration = registrations.first().expect("registered provider");
    let artifact = artifact_root
        .join("blake3-256")
        .join("a".repeat(64))
        .join(registration.binary());
    std::fs::create_dir_all(artifact.parent().expect("artifact generation"))
        .expect("create artifact generation");
    std::fs::write(&artifact, b"canonical-provider-binary").expect("write artifact");
    let profile = runtime_bin.join(registration.binary());
    std::os::unix::fs::symlink(&artifact, &profile).expect("link canonical provider binary");

    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .expect("accept canonical provider profile");

    assert_eq!(reconciliation.reconciled_count, 1);
    assert_eq!(reconciliation.changed_count, 0);
    assert_eq!(reconciliation.binary_byte_reads, 0);
    assert_eq!(
        std::fs::read_link(&profile).expect("canonical symlink remains unchanged"),
        artifact
    );

    std::fs::remove_dir_all(root).expect("remove canonical fixture");
}

#[cfg(unix)]
#[test]
fn unmanaged_provider_runtime_symlink_is_rejected_without_migration() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-unmanaged-runtime-entry-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");
    std::fs::create_dir_all(&artifact_root).expect("temporary artifact root");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let registration = registrations.first().expect("registered provider");
    let digest = "a".repeat(64);
    let unmanaged_binary = root
        .join("unmanaged")
        .join("blake3-256")
        .join(digest)
        .join(registration.binary());
    std::fs::create_dir_all(unmanaged_binary.parent().expect("unmanaged generation"))
        .expect("create unmanaged generation");
    std::fs::write(&unmanaged_binary, b"unmanaged-provider-binary")
        .expect("write unmanaged binary");
    let profile = runtime_bin.join(registration.binary());
    std::os::unix::fs::symlink(&unmanaged_binary, &profile)
        .expect("link unmanaged provider binary");

    let error = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .expect_err("reject unmanaged provider runtime symlink");

    assert!(
        error.contains("is not a canonical digest-lattice symlink"),
        "unexpected reconciliation error: {error}"
    );
    assert_eq!(
        std::fs::read_link(&profile).expect("unmanaged symlink remains untouched"),
        unmanaged_binary
    );

    std::fs::remove_dir_all(root).expect("remove migration fixture");
}
