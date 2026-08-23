use std::fs;
use std::process::Command;

use super::{make_executable, temp_project_root};

#[cfg(unix)]
#[test]
fn default_develop_receipt_install_publishes_installed_artifacts_atomically() {
    let root = temp_project_root();
    let state_home = root.join("state");
    let dev_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repository root");
    let source = dev_root
        .join("languages/rust-lang-project-harness/target")
        .join(format!("asp-install-receipt-test-{}", std::process::id()))
        .join("asp-rust");
    fs::create_dir_all(&state_home).expect("create ASP State Home");
    fs::write(
        state_home.join("asp.toml"),
        format!("[dev]\nenabled = true\nroot = {:?}\n", dev_root),
    )
    .expect("write development artifact authority");
    fs::create_dir_all(source.parent().expect("provider build parent"))
        .expect("create provider build parent");
    fs::write(&source, b"#!/bin/sh\nexit 0\n").expect("write provider fixture");
    make_executable(&source);

    let runtime_bin = state_home.join("runtime/bin");
    let provider_receipts = state_home.join("runtime/providers/receipts");
    fs::create_dir_all(&runtime_bin).expect("create runtime bin");
    fs::create_dir_all(&provider_receipts).expect("create provider receipt directory");
    for (language_id, provider_id, binary) in [
        ("typescript", "asp-typescript", "asp-typescript"),
        ("python", "asp-python", "asp-python"),
        ("julia", "asp-julia", "asp-julia"),
        ("gerbil-scheme", "asp-gerbil-scheme", "asp-gerbil-scheme"),
    ] {
        let binary_path = runtime_bin.join(binary);
        fs::write(
            &binary_path,
            format!("#!/bin/sh\n# {provider_id}\nexit 0\n").as_bytes(),
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
            provider_receipts.join(format!("{language_id}.lock.toml")),
            format!(
                "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"{}\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
                language_id,
                provider_id,
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
        first_receipt.contains("installedProviderArtifacts=sha256:"),
        "{first_receipt}"
    );
    assert!(
        first_receipt.contains("installedProviderArtifactsWrite=true"),
        "{first_receipt}"
    );
    let artifacts_path = state_home.join("runtime/installed-provider-artifacts.json");
    let artifacts: serde_json::Value = serde_json::from_slice(
        &fs::read(&artifacts_path).expect("read installed provider artifacts"),
    )
    .expect("decode installed provider artifacts");
    let provider = artifacts["providers"]
        .as_array()
        .expect("installed providers")
        .iter()
        .find(|provider| provider["languageId"] == "rust")
        .expect("Rust installed artifact leaf");
    let materialized_path = provider["materializedPath"]
        .as_str()
        .expect("materialized provider path");
    assert_eq!(
        std::path::Path::new(materialized_path),
        state_home.join("runtime/bin/asp-rust"),
        "installed artifacts must publish the stable Runtime lattice entry"
    );
    assert_eq!(
        fs::canonicalize(materialized_path).expect("resolve development lattice entry"),
        fs::canonicalize(
            dev_root.join("languages/rust-lang-project-harness/target/release/asp-rust"),
        )
        .expect("resolve registered workspace artifact"),
        "development install must select the registry-owned workspace artifact"
    );
    assert!(
        std::path::Path::new(materialized_path).is_file(),
        "installed provider artifact is missing: {materialized_path}"
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
        second_receipt.contains("installedProviderArtifactsWrite=false"),
        "{second_receipt}"
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(source.parent().expect("provider fixture parent"));
}

#[test]
fn root_justfile_develop_install_is_only_a_registry_driven_adapter() {
    let justfile = include_str!("../../../../../Justfile");

    for required in [
        r#"protocol_bin="${state_home}/runtime/bin/asp""#,
        r#"install language "{{ language }}""#,
        "installMode=provider-workspace-install source=provider-registry",
    ] {
        assert!(
            justfile.contains(required),
            "developer provider adapter lost its registry contract: {required}"
        );
    }
    for forbidden in [
        "--record-installed-receipt",
        "provider_source=",
        r#"case "{{ language }}" in"#,
        "languages/rust-lang-project-harness/target/release/asp-rust",
        "runtime/provider-artifacts/asp-gerbil-scheme/develop",
    ] {
        assert!(
            !justfile.contains(forbidden),
            "root Justfile duplicated provider-owned install logic: {forbidden}"
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
            "asp-rust",
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
