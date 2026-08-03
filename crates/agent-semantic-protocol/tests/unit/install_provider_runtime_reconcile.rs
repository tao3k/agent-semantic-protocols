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
fn legacy_digest_symlink_is_migrated_to_lattice_profile_file() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-lattice-migration-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let registration = registrations.first().expect("registered provider");
    let digest = "a".repeat(64);
    let legacy_binary = artifact_root
        .join("blake3-256")
        .join(digest)
        .join(registration.binary());
    std::fs::create_dir_all(legacy_binary.parent().expect("legacy generation"))
        .expect("create legacy generation");
    std::fs::write(&legacy_binary, b"legacy-provider-binary").expect("write legacy binary");
    let profile = runtime_bin.join(registration.binary());
    std::os::unix::fs::symlink(&legacy_binary, &profile).expect("link legacy provider binary");

    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .expect("migrate legacy provider profile");

    assert_eq!(reconciliation.registration_count, 1);
    assert_eq!(reconciliation.reconciled_count, 1);
    assert_eq!(reconciliation.changed_count, 1);
    assert!(
        std::fs::symlink_metadata(&profile)
            .expect("inspect migrated profile")
            .file_type()
            .is_file()
    );
    assert_eq!(
        std::fs::read(&profile).expect("read migrated profile"),
        b"legacy-provider-binary"
    );

    std::fs::remove_dir_all(root).expect("remove migration fixture");
}
