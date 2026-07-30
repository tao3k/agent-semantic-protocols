use super::{
    asset_name, checksum_name, parse_sha256_checksum, path_segment, provider_release,
    validate_target,
};

#[test]
fn asset_names_are_rev_independent_and_target_selected() {
    let spec = provider_release("julia").expect("julia release spec");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz.sha256"
    );
}

#[test]
fn registered_provider_receipt_covers_language_alias_for_same_binary() {
    let receipt = super::super::install_provider_reconcile::ProviderInstallReceipt {
        language_id: "org".to_string(),
        provider_id: "orgize".to_string(),
        installed_path: std::path::PathBuf::from("/runtime/bin/orgize"),
        installed_entrypoint_digest: "content".to_string(),
        installed_entrypoint_metadata_digest: "metadata".to_string(),
        execution_command_digest: "execution".to_string(),
    };

    assert!(super::registered_provider_receipt_covers_binary(
        &[receipt],
        "orgize"
    ));
}

#[test]
fn orgize_release_pin_resolves_provider_binary_asset() {
    let spec = provider_release("org").expect("orgize release spec");
    assert_eq!(spec.provider_id, "orgize");
    assert_eq!(spec.binary, "orgize");
    assert_eq!(spec.release_version, "v0.10.0-alpha.10");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "orgize-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "orgize-aarch64-apple-darwin.tar.gz.sha256"
    );
    assert_eq!(
        super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
            .expect("valid orgize release sha256"),
        Some("bf29b471d01529f5a98364dddac8aa24d5d57f31ba794e3b94e591d8527f9cf8")
    );
}

#[test]
fn pinned_release_hash_requires_a_valid_value_for_each_target() {
    let mut spec = provider_release("org").expect("orgize release spec");
    spec.sha256_by_target.remove("aarch64-apple-darwin");
    let missing = super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
        .expect_err("missing target hash must fail");
    assert!(
        missing.contains("missing pinned release sha256"),
        "{missing}"
    );

    spec.sha256_by_target.insert(
        "aarch64-apple-darwin".to_string(),
        "NOT-A-SHA256".to_string(),
    );
    let invalid = super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
        .expect_err("invalid target hash must fail");
    assert!(
        invalid.contains("invalid pinned release sha256"),
        "{invalid}"
    );
}

#[test]
fn parse_checksum_accepts_common_sha256_formats() {
    assert_eq!(
        parse_sha256_checksum(
            "ABCDEFabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123  file.tar.gz\n"
        )
        .as_deref(),
        Some("abcdefabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123")
    );
}

#[test]
fn rev_path_segment_is_filesystem_safe() {
    assert_eq!(
        path_segment("refs/tags/v1.2.3+build"),
        "refs_tags_v1.2.3_build"
    );
}

#[test]
fn unsupported_apple_intel_target_is_rejected() {
    let spec = provider_release("rust").expect("rust release spec");
    let error = validate_target(&spec, "x86_64-apple-darwin").expect_err("unsupported target");
    assert!(error.contains("unsupported target `x86_64-apple-darwin`"));
    assert!(error.contains("aarch64-apple-darwin"));
}
fn install_scope_args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn install_language_defaults_to_global_scope() {
    let parsed = super::parse_install_args(&[]).expect("default global install scope");
    assert_eq!(parsed.scope, super::InstallScope::Global);
}

#[test]
fn install_language_accepts_explicit_global_scope() {
    let parsed =
        super::parse_install_args(&install_scope_args(&["--global"])).expect("global scope");
    assert_eq!(parsed.scope, super::InstallScope::Global);
}

#[test]
fn install_language_requires_explicit_canonical_project_scope() {
    let current = std::env::current_dir()
        .expect("current directory")
        .canonicalize()
        .expect("canonical current directory");
    let parsed = super::parse_install_args(&install_scope_args(&[
        "--project",
        current.to_str().expect("UTF-8 project root"),
    ]))
    .expect("project scope");
    assert_eq!(
        parsed.scope,
        super::InstallScope::Project {
            root: current.clone()
        }
    );

    let conflict = super::parse_install_args(&install_scope_args(&["--global", "--project", "."]))
        .expect_err("global and project must conflict");
    assert!(conflict.contains("cannot be used with"));
}

#[test]
fn install_language_rejects_positional_and_workspace_legacy_scope() {
    assert!(super::parse_install_args(&install_scope_args(&["."])).is_err());
    assert!(super::parse_install_args(&install_scope_args(&["--workspace", "."])).is_err());
}

#[test]
fn global_provider_state_is_separate_from_runtime_bin_and_project_state() {
    let state_home = std::env::temp_dir().join(format!(
        "asp-install-scope-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    let provider_root = super::canonical_global_provider_state_root_from(&state_home)
        .expect("global provider root");
    assert_eq!(
        provider_root,
        state_home
            .canonicalize()
            .expect("canonical state home")
            .join("runtime")
            .join("providers")
    );
    assert_ne!(provider_root, state_home.join("runtime").join("bin"));
    std::fs::remove_dir_all(state_home).expect("remove isolated test state");
}
