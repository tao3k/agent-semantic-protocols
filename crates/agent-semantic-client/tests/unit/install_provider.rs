use super::ProviderArtifactAuthority;
use super::ProviderInstallLock;
use super::asset_name;
use super::checksum_name;
use super::parse_sha256_checksum;
use super::path_segment;
use super::provider_release;
use super::validate_target;
use super::write_provider_lock;

#[test]
fn provider_lock_serializes_canonical_artifact_digest_without_legacy_generation_key() {
    let root = std::env::temp_dir().join(format!(
        "asp-provider-lock-artifact-identity-{}",
        std::process::id()
    ));
    let path = root.join("python.lock.toml");
    let artifact = root.join("asp-python");
    write_provider_lock(
        &path,
        &ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            scope: "state-home",
            language_id: "python",
            provider_id: "asp-python",
            source_kind: "develop-workspace-tree",
            checkout_root: Some(&root),
            provider_source_root: Some(&root),
            repo: None,
            rev: None,
            target: "aarch64-apple-darwin",
            binary: "asp-python",
            installed_path: &artifact,
            package_path: &root,
            sha256: "fixture-sha256",
            source: root.display().to_string(),
            source_snapshot_root: None,
            source_snapshot_algorithm: None,
            source_leaf_count: None,
            provider_digest: None,
            build_recipe_digest: None,
            artifact_digest: Some("blake3-256:artifact"),
            artifact_leaf_count: None,
            artifact_entrypoint: None,
            artifact_entrypoint_sha256: None,
            installed_entrypoint_digest: Some("blake3-256:entrypoint"),
            installed_entrypoint_metadata_digest: "blake3-256:metadata",
            execution_command_digest: "blake3-256:command",
            launcher_digest: None,
        },
    )
    .expect("write canonical provider lock");
    let contents = std::fs::read_to_string(&path).expect("read provider lock");
    assert!(contents.contains("artifactDigest = \"blake3-256:artifact\""));
    assert!(!contents.contains("binarySourceGeneration"));
    std::fs::remove_dir_all(root).expect("remove provider lock fixture");
}

#[test]
fn asset_names_are_rev_independent_and_target_selected() {
    let spec = provider_release("julia").expect("julia release spec");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-aarch64-apple-darwin.tar.gz.sha256"
    );
}

#[test]
fn pinned_release_hash_requires_a_valid_value_for_each_target() {
    let mut spec = provider_release("rust").expect("Rust release spec");
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

#[test]
fn external_register_fixture_resolves_identity_without_caller_supplied_binary() {
    let registration = agent_semantic_provider_protocol::ProviderRegistrationDocument {
        language_id: "external-language".to_string(),
        provider_id: "external-provider".to_string(),
        registration: serde_json::json!({
            "languageId": "external-language",
            "providerId": "external-provider"
        }),
    };
    // The provider register owns identity; release metadata owns archive naming.
    let provider_id = super::canonical_provider_identity("external-language", &[registration])
        .expect("external provider identity");
    assert_eq!(provider_id, "external-provider");
}
#[test]
fn provider_state_home_is_separate_from_runtime_bin() {
    let state_home = std::env::temp_dir().join(format!(
        "asp-install-scope-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    let provider_root =
        super::canonical_provider_state_root_from(&state_home).expect("State Home provider root");
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

#[test]
fn developer_mode_never_selects_locked_release_or_path_fallback() {
    let root = std::path::PathBuf::from("/checkout/agent-semantic-protocols");
    let mode = agent_semantic_config::runtime_dev::parse_runtime_artifact_mode(&format!(
        "[dev]\nenabled = true\nroot = {:?}\n",
        root
    ))
    .expect("parse developer runtime mode");
    assert_eq!(
        super::provider_artifact_authority(&mode).expect("dev build authority"),
        ProviderArtifactAuthority::DevelopBuild {
            root: root.as_path(),
        }
    );
}

#[test]
fn release_mode_selects_only_locked_release() {
    let mode = agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release;
    assert_eq!(
        super::provider_artifact_authority(&mode).expect("locked release"),
        ProviderArtifactAuthority::LockedRelease
    );
}
