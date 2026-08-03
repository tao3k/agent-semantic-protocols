use std::fs;
use std::process::Command;

use super::{make_executable, temp_project_root};

#[cfg(unix)]
#[test]
fn default_develop_receipt_install_publishes_the_global_runtime_catalog_atomically() {
    let root = temp_project_root();
    let state_home = root.join("state");
    let source = root.join("build/rs-harness");
    fs::create_dir_all(source.parent().expect("provider build parent"))
        .expect("create provider build parent");
    fs::write(&source, b"#!/bin/sh\nexit 0\n").expect("write provider fixture");
    make_executable(&source);

    let runtime_bin = state_home.join("runtime/bin");
    let provider_receipts = state_home.join("runtime/providers/receipts");
    fs::create_dir_all(&runtime_bin).expect("create runtime bin");
    fs::create_dir_all(&provider_receipts).expect("create provider receipt directory");
    for manifest in agent_semantic_hook::schema_registry_provider_manifests()
        .into_iter()
        .filter(|manifest| manifest.language_id().as_str() != "rust")
    {
        let binary_path = runtime_bin.join(manifest.binary());
        fs::write(
            &binary_path,
            format!("#!/bin/sh\n# {}\nexit 0\n", manifest.provider_id()).as_bytes(),
        )
        .expect("write registered provider fixture");
        make_executable(&binary_path);
        let content_digest = agent_semantic_content_identity::file_content_digest_v1(&binary_path)
            .expect("provider content digest");
        let metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(&binary_path)
                .expect("provider metadata digest");
        let execution_digest = agent_semantic_hook::provider_execution_command_digest(
            &[binary_path.to_string_lossy().to_string()],
            &content_digest,
        )
        .expect("provider execution command digest");
        fs::write(
            provider_receipts.join(format!("{}.lock.toml", manifest.language_id())),
            format!(
                "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
                manifest.language_id(),
                manifest.provider_id(),
                binary_path.display(),
                content_digest,
                metadata_digest,
                execution_digest,
            ),
        )
        .expect("write registered provider receipt");
    }

    let install = || {
        Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "install",
                "language",
                "rust",
                "--record-installed-receipt",
                source.to_str().expect("utf-8 provider source"),
            ])
            .env("ASP_STATE_HOME", &state_home)
            .env("ASP_NO_AGENT_PLATFORM", "1")
            .output()
            .expect("publish develop provider receipt")
    };

    let first = install();
    assert!(
        first.status.success(),
        "{}{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first_receipt = String::from_utf8_lossy(&first.stdout);
    assert!(first_receipt.contains("scope=global"), "{first_receipt}");
    assert!(
        first_receipt.contains("globalProviderCatalog=blake3-256:"),
        "{first_receipt}"
    );
    assert!(
        first_receipt.contains("globalProviderCatalogWrite=true"),
        "{first_receipt}"
    );

    let catalog_path = state_home.join("runtime/provider-catalog.v1.json");
    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).expect("read global provider catalog"))
            .expect("decode global provider catalog");
    let provider = catalog["providers"]
        .as_array()
        .expect("catalog providers")
        .iter()
        .find(|provider| provider["languageId"] == "rust")
        .expect("Rust provider catalog leaf");
    let materialized_path = provider["materializedPath"]
        .as_str()
        .expect("materialized provider path");
    let canonical_artifact_root =
        fs::canonicalize(state_home.join("runtime/artifacts")).expect("canonical artifact root");
    assert!(
        std::path::Path::new(materialized_path).starts_with(&canonical_artifact_root),
        "provider escaped ASP State Home runtime artifacts: {materialized_path}"
    );
    assert_ne!(
        materialized_path,
        source.to_str().expect("utf-8 provider source")
    );
    assert_eq!(
        provider["argvPrefix"][0]
            .as_str()
            .expect("provider argv prefix"),
        materialized_path
    );
    assert!(
        std::path::Path::new(materialized_path).is_file(),
        "catalog provider artifact is missing: {materialized_path}"
    );

    let second = install();
    assert!(
        second.status.success(),
        "{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    let second_receipt = String::from_utf8_lossy(&second.stdout);
    assert!(
        second_receipt.contains("globalProviderCatalogWrite=false"),
        "{second_receipt}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn root_justfile_develop_install_has_one_state_home_runtime_authority() {
    let justfile = include_str!("../../../../../Justfile");

    for required in [
        r#"runtime_bin="${state_home}/runtime/bin""#,
        r#"provider_source="${repo_root}/languages/rust-lang-project-harness/target/release/rs-harness""#,
        r#"--record-installed-receipt "${provider_source}""#,
        r#"protocol_bin="${state_home}/runtime/bin/asp""#,
        "custom provider bin_dir is unsupported; ASP State Home runtime/bin is the only provider runtime authority",
    ] {
        assert!(
            justfile.contains(required),
            "developer provider install lost the single-runtime contract: {required}"
        );
    }
}

#[test]
fn record_and_reconcile_receipt_modes_are_mutually_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--record-installed-receipt",
            "rs-harness",
            "--reconcile-receipt",
        ])
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("run conflicting receipt modes");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains(
            "the argument '--record-installed-receipt <PATH>' cannot be used with '--reconcile-receipt'"
        ),
        "{receipt}"
    );
}
