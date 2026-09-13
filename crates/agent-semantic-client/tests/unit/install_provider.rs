// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ProviderArtifactAuthority;
use super::ProviderInstallLock;
use super::asset_name;
use super::path_segment;
use super::provider_release;
use super::validate_target;
use super::write_provider_lock;

#[test]
fn active_provider_reconciliation_without_an_active_bundle_is_empty_and_offline() {
    let state_home = tempfile::tempdir().expect("state home");
    let plan = super::prepare_active_provider_reconciliation(state_home.path())
        .expect("empty active-provider reconciliation");
    assert_eq!(plan.provider_count(), 0);
    assert!(plan.member_sources().is_empty());
    assert!(!plan.staging_root.exists());
}

#[tokio::test]
async fn binary_refresh_builds_a_complete_bound_closure_for_an_empty_provider_set() {
    let state_home = tempfile::tempdir().expect("state home");
    let sources = tempfile::tempdir().expect("binary sources");
    let asp = sources.path().join("asp");
    let hook = sources.path().join("asp-hook");
    std::fs::write(&asp, b"asp").expect("asp candidate");
    std::fs::write(&hook, b"hook").expect("hook candidate");
    let mut plan = super::prepare_active_provider_reconciliation(state_home.path())
        .expect("empty active-provider reconciliation");

    let binding = plan
        .bind_runtime_execution_closure(&asp, &hook)
        .await
        .expect("bound Runtime execution closure");

    assert_eq!(plan.provider_count(), 0);
    let members = plan.member_sources();
    assert_eq!(members.len(), 5);
    let registration = members
        .iter()
        .find(|member| member.name == "provider-registration.json")
        .expect("provider registration member");
    let registration_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_digest(
            registration.source,
        )
        .await
        .expect("registration digest");
    assert_eq!(binding.provider_registration_digest(), &registration_digest);
    let registration_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(registration.source).expect("registration bytes"))
            .expect("registration JSON");
    assert_eq!(registration_json["entries"], serde_json::json!([]));
}

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
        "asp-julia-harness-aarch64-apple-darwin.tar.gz"
    );
}

#[test]
fn pinned_release_hash_requires_a_valid_value_for_each_target() {
    let mut spec = provider_release("rust").expect("Rust release spec");
    spec.sha256_by_target.remove("aarch64-apple-darwin");
    let missing = super::pinned_release_sha256(&spec, "aarch64-apple-darwin")
        .expect_err("missing target hash must fail");
    assert!(
        missing.contains("provider-release-target-digest-missing"),
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
fn provider_install_inputs_are_owned_by_artifact_staging() {
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
            .join("artifacts")
            .join("staging")
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
