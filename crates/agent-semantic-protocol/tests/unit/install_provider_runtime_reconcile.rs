use super::{
    prune_stale_registered_provider_leaves, reconcile_registered_provider_runtime_binaries_from,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[cfg(unix)]
fn stale_receipt(language: &str, installed_path: &std::path::Path) -> String {
    format!(
        "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{language}\"\nprovider = \"legacy-{language}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
        installed_path.display(),
        "a".repeat(64),
        "b".repeat(64),
        "c".repeat(64)
    )
}

#[cfg(unix)]
#[tokio::test]
async fn stale_missing_legacy_catalog_entry_is_pruned_before_provider_resolution() {
    let root = std::env::temp_dir().join(format!("asp-stale-catalog-{}", std::process::id()));
    let bin = root.join("runtime/bin");
    let receipts = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&receipts).unwrap();
    let legacy = bin.join("obsolete-typescript-provider");
    std::fs::write(&legacy, b"legacy").unwrap();
    std::fs::write(
        receipts.join("typescript.lock.toml"),
        stale_receipt("typescript", &legacy),
    )
    .unwrap();
    prune_stale_registered_provider_leaves(&[], &bin, &receipts)
        .await
        .unwrap();
    assert!(!legacy.exists());
    assert!(!receipts.join("typescript.lock.toml").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn malicious_external_provider_sentinel_is_untouched_during_legacy_prune() {
    let root = std::env::temp_dir().join(format!("asp-external-sentinel-{}", std::process::id()));
    let bin = root.join("runtime/bin");
    let receipts = root.join("runtime/providers/receipts");
    let external = root.join("external-sentinel");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&receipts).unwrap();
    std::fs::write(&external, b"must survive").unwrap();
    let legacy = bin.join("obsolete-typescript-provider");
    std::os::unix::fs::symlink(&external, &legacy).unwrap();
    std::fs::write(
        receipts.join("typescript.lock.toml"),
        stale_receipt("typescript", &legacy),
    )
    .unwrap();
    prune_stale_registered_provider_leaves(&[], &bin, &receipts)
        .await
        .unwrap();
    assert!(external.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn current_provider_missing_artifact_returns_typed_missing_result() {
    let root = std::env::temp_dir().join(format!("asp-current-missing-{}", std::process::id()));
    let bin = root.join("runtime/bin");
    let artifacts = root.join("runtime/artifacts");
    let receipts = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::create_dir_all(&receipts).unwrap();
    let registration = agent_semantic_hook::registered_provider_binaries_v1()
        .into_iter()
        .next()
        .unwrap();
    let result = reconcile_registered_provider_runtime_binaries_from(
        &[registration],
        &bin,
        &artifacts,
        &receipts,
    )
    .await
    .unwrap();
    assert_eq!(result.missing_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_reconcile_readers_observe_only_complete_old_or_new_snapshot() {
    let root =
        std::env::temp_dir().join(format!("asp-concurrent-reconcile-{}", std::process::id()));
    let bin = root.join("runtime/bin");
    let receipts = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&receipts).unwrap();
    let a = prune_stale_registered_provider_leaves(&[], &bin, &receipts);
    let b = prune_stale_registered_provider_leaves(&[], &bin, &receipts);
    let (a, b) = tokio::join!(a, b);
    assert!(a.is_ok() && b.is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn registered_scheme_and_python_dangling_runtime_links_are_missing() {
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
        .await
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
#[tokio::test]
async fn canonical_digest_lattice_symlink_is_current_without_rewrite() {
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
    std::fs::create_dir_all(&provider_lock_dir).expect("temporary provider receipts");
    let content_digest =
        agent_semantic_content_identity::file_content_digest_v1(&profile).expect("content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&profile)
            .expect("metadata digest");
    let execution_digest = agent_semantic_hook::provider_execution_command_digest(
        &[profile.to_string_lossy().to_string()],
        &content_digest,
    )
    .expect("execution digest");
    let lock_path =
        provider_lock_dir.join(format!("{}.lock.toml", registration.language_id().as_str()));
    std::fs::write(
        &lock_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"legacy-provider-id\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
            registration.language_id().as_str(),
            profile.display(),
            content_digest,
            metadata_digest,
            execution_digest,
        ),
    )
    .expect("write legacy identity provider receipt");

    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .await
    .expect("accept canonical provider profile");

    assert_eq!(reconciliation.reconciled_count, 1);
    assert_eq!(reconciliation.changed_count, 0);
    assert_eq!(reconciliation.receipt_changed_count, 1);
    assert_eq!(reconciliation.binary_byte_reads, 1);
    let receipt = super::super::install_provider_reconcile::read_provider_install_receipt(
        registration.language_id().as_str(),
        &provider_lock_dir,
    )
    .expect("read migrated provider receipt");
    assert_eq!(receipt.provider_id, registration.provider_id().as_str());
    assert_eq!(
        std::fs::read_link(&profile).expect("canonical symlink remains unchanged"),
        artifact
    );

    std::fs::remove_dir_all(root).expect("remove canonical fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn registered_provider_file_is_atomically_migrated_to_the_digest_lattice() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-file-runtime-entry-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let registration = registrations.first().expect("registered provider");
    let profile = runtime_bin.join(registration.binary());
    std::fs::write(&profile, b"provider-binary-awaiting-lattice-migration")
        .expect("write registered provider file");
    std::fs::create_dir_all(&provider_lock_dir).expect("temporary provider receipts");
    let content_digest =
        agent_semantic_content_identity::file_content_digest_v1(&profile).expect("content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&profile)
            .expect("metadata digest");
    let execution_digest = agent_semantic_hook::provider_execution_command_digest(
        &[profile.to_string_lossy().to_string()],
        &content_digest,
    )
    .expect("execution digest");
    let lock_path =
        provider_lock_dir.join(format!("{}.lock.toml", registration.language_id().as_str()));
    std::fs::write(
        &lock_path,
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
            registration.language_id().as_str(),
            registration.provider_id().as_str(),
            profile.display(),
            content_digest,
            metadata_digest,
            execution_digest,
        ),
    )
    .expect("write pre-migration provider receipt");

    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .await
    .expect("migrate registered provider file");

    assert_eq!(reconciliation.reconciled_count, 1);
    assert_eq!(reconciliation.changed_count, 1);
    assert_eq!(reconciliation.receipt_reconciled_count, 1);
    assert_eq!(reconciliation.receipt_changed_count, 1);
    assert_eq!(reconciliation.binary_byte_reads, 2);
    assert!(
        std::fs::symlink_metadata(&profile)
            .expect("migrated provider profile")
            .file_type()
            .is_symlink()
    );
    let canonical = std::fs::canonicalize(&profile).expect("canonical provider artifact");
    let canonical_artifact_root =
        std::fs::canonicalize(&artifact_root).expect("canonical artifact root");
    assert!(canonical.starts_with(canonical_artifact_root.join("blake3-256")));
    assert_eq!(
        std::fs::read(&canonical).expect("read migrated artifact"),
        b"provider-binary-awaiting-lattice-migration"
    );
    let reconciled_receipt =
        super::super::install_provider_reconcile::read_provider_install_receipt(
            registration.language_id().as_str(),
            &provider_lock_dir,
        )
        .expect("read reconciled provider receipt");
    assert!(
        super::super::install_provider_reconcile::provider_install_receipt_matches_artifact(
            &reconciled_receipt,
            &profile,
        )
        .expect("receipt matches migrated provider artifact")
    );

    std::fs::remove_dir_all(root).expect("remove migration fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn stale_provider_generation_is_replaced_from_verified_developer_output() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-runtime-developer-reconcile-{}-{nonce}",
        std::process::id()
    ));
    let developer_root = root.join("checkout");
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("temporary runtime bin");
    std::fs::create_dir_all(&artifact_root).expect("temporary artifact root");
    std::fs::create_dir_all(&provider_lock_dir).expect("temporary provider receipts");
    std::fs::write(
        root.join("asp.toml"),
        format!(
            "[dev]\nenabled = true\nroot = {:?}\n",
            developer_root.display().to_string()
        ),
    )
    .expect("write Developer Source config");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let registration = registrations
        .iter()
        .find(|registration| registration.language_id().as_str() == "rust")
        .expect("registered Rust provider");
    let development = agent_semantic_hook::registered_provider_development_v1("rust")
        .expect("registered Rust development descriptor");
    let provider_source_root = developer_root.join(&development.development.source_root);
    let descriptor_path = provider_source_root.join(
        development
            .development
            .workspace_install
            .as_deref()
            .expect("Rust workspace install descriptor"),
    );
    let verified_output = provider_source_root.join("target/release/asp-rust");
    std::fs::create_dir_all(descriptor_path.parent().expect("descriptor parent"))
        .expect("create descriptor parent");
    std::fs::create_dir_all(verified_output.parent().expect("output parent"))
        .expect("create output parent");
    std::fs::write(&verified_output, b"verified Developer Source provider")
        .expect("write verified provider output");
    std::fs::write(
        &descriptor_path,
        format!(
            r#"{{
                "schemaId":"agent.semantic-protocols.provider-workspace-install",
                "schemaVersion":"1",
                "schemaAuthority":"https://tao3k.github.io/agent-semantic-protocols/schemas/",
                "languageId":"rust",
                "providerId":"{}",
                "binary":"{}",
                "workspaceArtifact":{{
                    "root":"{}/target/release/asp-rust",
                    "entrypoint":"."
                }},
                "workspaceBuild":{{
                    "derivedPaths":["{}/target/release"]
                }}
            }}"#,
            registration.provider_id().as_str(),
            registration.binary(),
            development.development.source_root,
            development.development.source_root,
        ),
    )
    .expect("write workspace install descriptor");

    let old_generation = artifact_root.join("blake3-256/old/asp-rust");
    std::fs::create_dir_all(old_generation.parent().expect("old generation parent"))
        .expect("create old generation");
    std::fs::write(&old_generation, b"stale provider").expect("write stale provider");
    symlink(&old_generation, runtime_bin.join("asp-rust")).expect("link stale provider");

    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .await
    .expect("auto-update stale provider generation");

    assert_eq!(reconciliation.changed_count, 1);
    assert_eq!(
        std::fs::canonicalize(runtime_bin.join("asp-rust")).expect("canonical runtime provider"),
        std::fs::canonicalize(&verified_output).expect("canonical Developer output")
    );

    std::fs::remove_dir_all(root).expect("remove Developer reconciliation fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn stale_typescript_receipt_and_catalog_are_replaced_by_verified_developer_output() {
    let root = std::env::temp_dir().join(format!("asp-ts-stale-catalog-{}", std::process::id()));
    let developer_root = root.join("checkout");
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).unwrap();
    std::fs::create_dir_all(&artifact_root).unwrap();
    std::fs::create_dir_all(&provider_lock_dir).unwrap();
    std::fs::write(
        root.join("asp.toml"),
        format!(
            "[dev]\nenabled = true\nroot = {:?}\n",
            developer_root.display().to_string()
        ),
    )
    .unwrap();
    let registration = agent_semantic_hook::registered_provider_binaries_v1()
        .into_iter()
        .find(|r| r.language_id().as_str() == "typescript")
        .unwrap();
    let development =
        agent_semantic_hook::registered_provider_development_v1("typescript").unwrap();
    let source_root = developer_root.join(&development.development.source_root);
    let workspace = source_root.join(
        development
            .development
            .workspace_install
            .as_deref()
            .unwrap(),
    );
    std::fs::create_dir_all(workspace.parent().unwrap()).unwrap();
    let authority = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../languages/typescript-lang-project-harness/provider/asp-provider-workspace-install.json");
    std::fs::write(&workspace, std::fs::read(&authority).unwrap()).unwrap();
    let verified = source_root.join("dist/src/cli/main.js");
    std::fs::create_dir_all(verified.parent().unwrap()).unwrap();
    std::fs::write(&verified, b"verified asp-typescript").unwrap();
    let stale = runtime_bin.join("obsolete-typescript-provider");
    std::fs::write(&stale, b"stale").unwrap();
    std::fs::write(
        provider_lock_dir.join("typescript.lock.toml"),
        stale_receipt("typescript", &stale),
    )
    .unwrap();
    let reconciliation = reconcile_registered_provider_runtime_binaries_from(
        std::slice::from_ref(&registration),
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .await
    .unwrap();
    let receipt = reconciliation
        .provider_receipts
        .iter()
        .find(|r| r.language_id == "typescript")
        .unwrap();
    assert_eq!(receipt.provider_id, "asp-typescript");
    assert_eq!(receipt.installed_path, verified);
    let stable = runtime_bin.join("asp-typescript");
    assert_eq!(std::fs::read_link(&stable).unwrap(), receipt.installed_path);
    assert!(!stale.exists());
    let readiness =
        super::publish_registered_provider_runtime_reconciliation(&root, &reconciliation)
            .await
            .unwrap();
    let catalog = std::fs::read_to_string(root.join("runtime/provider-catalog.v1.json")).unwrap();
    assert!(catalog.contains("asp-typescript"));
    assert!(catalog.contains(&receipt.installed_entrypoint_digest));
    assert!(catalog.contains(stable.to_string_lossy().as_ref()));
    assert_eq!(readiness.provider_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn unmanaged_provider_runtime_symlink_is_rejected_without_migration() {
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
    .await
    .expect_err("reject unmanaged provider runtime symlink");

    assert!(
        error.contains("escapes immutable artifact root"),
        "unexpected reconciliation error: {error}"
    );
    assert_eq!(
        std::fs::read_link(&profile).expect("unmanaged symlink remains untouched"),
        unmanaged_binary
    );

    std::fs::remove_dir_all(root).expect("remove migration fixture");
}
