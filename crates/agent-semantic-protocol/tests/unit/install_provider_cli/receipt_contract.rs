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
    let artifact_root = state_home.join("runtime/artifacts/blake3-256");
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
        let artifact_path = artifact_root.join(language_id).join(binary);
        fs::create_dir_all(artifact_path.parent().expect("provider artifact parent"))
            .expect("create provider CAS fixture");
        fs::write(
            &artifact_path,
            format!("#!/bin/sh\n# {provider_id}\nexit 0\n").as_bytes(),
        )
        .expect("write registered provider fixture");
        make_executable(&artifact_path);
        std::os::unix::fs::symlink(&artifact_path, &binary_path)
            .expect("publish registered provider CAS entry");
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
            .env("ASP_NO_AGENT", "1")
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
    let materialized_path = std::path::Path::new(materialized_path);
    assert!(
        materialized_path.starts_with(state_home.join("runtime/artifacts/blake3-256")),
        "installed artifacts must publish an immutable CAS entry: {}",
        materialized_path.display()
    );
    assert_eq!(
        materialized_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some("asp-rust"),
        "installed artifact identity must retain the provider binary name"
    );
    assert_eq!(
        fs::read(materialized_path).expect("read immutable development artifact"),
        fs::read(&source).expect("read explicitly selected development artifact"),
        "development install must publish the selected source artifact content"
    );
    assert!(
        materialized_path.is_file(),
        "installed provider artifact is missing: {}",
        materialized_path.display()
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
