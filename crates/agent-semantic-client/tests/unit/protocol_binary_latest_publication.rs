// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ProtocolBinaryInstallPlan;
use super::SEMANTIC_AGENT_PROTOCOL_BIN;
use super::ensure_protocol_binary_installed;
use super::install_protocol_binary_target;
use super::next_protocol_binary_publish_sequence;
use super::protocol_binary_artifact_path_digest;
use super::qualified_provider_state_home;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;

fn fixture_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!(
        "asp-protocol-binary-{name}-{}-{}",
        process::id(),
        next_protocol_binary_publish_sequence()
    ));
    fs::create_dir_all(&root).expect("create protocol binary fixture");
    root
}

#[tokio::test]
async fn published_runtime_identity_lookup_never_reads_artifact_bytes() {
    let root = fixture_root("published-runtime-identity");
    let artifact_root = root.join("runtime/artifacts");
    let target = root.join("home/.local/bin/asp");
    let source = fixture_source(&root, "source-asp", b"control-digest-fixture");
    let installed = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("publish digest-addressed ASP artifact");
    commit_pending_runtime_activation(&root).await;

    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = std::time::Instant::now();
        let digest = protocol_binary_artifact_path_digest(&target)
            .expect("read published artifact path identity without binary hashing");
        samples.push(started.elapsed());
        assert_eq!(
            digest,
            installed
                .bundle_digest
                .as_deref()
                .expect("published bundle digest")
                .strip_prefix("blake3-256:")
                .expect("canonical bundle digest")
        );
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-binary-published-identity] requests=10000 p99Nanos={} binaryByteReads=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "published runtime identity lookup p99 must remain sub-millisecond, observed {p99:?}"
    );

    fs::remove_dir_all(root).expect("remove protocol binary fixture");
}

fn fixture_source(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let source = root.join(name);
    fs::write(&source, bytes).expect("write protocol binary fixture");
    source
}

async fn commit_pending_runtime_activation(state_home: &Path) {
    let event =
        agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
            state_home,
        )
        .await
        .expect("read pending Runtime activation")
        .expect("pending Runtime activation event");
    agent_semantic_artifacts::runtime_artifact_activation::commit_runtime_artifact_activation(
        state_home, &event, None,
    )
    .await
    .expect("commit pending Runtime activation");
}

use super::RuntimeBinaryIdentityV1;

#[tokio::test]
async fn runtime_publication_rejects_target_name_inference_and_path_shaped_identities() {
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("../asp-rust").is_err());
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("bin/asp-rust").is_err());

    let root = fixture_root("declared-binary-mismatch");
    let artifact_root = root.join("runtime/artifacts");
    let source = fixture_source(&root, "source-asp-rust", b"asp-rust-v1");
    let wrong_target = root.join("runtime/bin/not-asp-rust");
    let identity = RuntimeBinaryIdentityV1::from_registered_provider("asp-rust")
        .expect("registered harness binary identity");

    let error = install_protocol_binary_target(&source, &wrong_target, &artifact_root, &identity)
        .await
        .expect_err("target filename must not override ProviderRegistry identity");
    assert!(error.contains("does not match declared binary identity"));

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[tokio::test]
async fn registered_provider_install_requires_an_active_complete_runtime_bundle() {
    let registrations = agent_semantic_provider_protocol::builtin_provider_registrations()
        .expect("builtin provider registrations");
    for (language_id, binary) in [
        ("gerbil-scheme", "asp-gerbil-scheme"),
        ("python", "asp-python"),
    ] {
        let registration = registrations
            .iter()
            .find(|registration| registration.language_id.as_str() == language_id)
            .unwrap_or_else(|| panic!("registered language `{language_id}`"));
        let provider_id = registration.provider_id.as_str();
        let root = fixture_root(provider_id);
        let artifact_root = root.join("runtime/artifacts");
        let target = root.join("runtime/bin").join(binary);
        std::fs::create_dir_all(target.parent().expect("runtime bin")).expect("create runtime bin");
        std::os::unix::fs::symlink(
            root.join("missing-provider-artifacts").join(provider_id),
            &target,
        )
        .unwrap_or_else(|error| panic!("dangling runtime link for `{provider_id}`: {error}"));
        let source = fixture_source(
            &root,
            &format!("source-{provider_id}"),
            provider_id.as_bytes(),
        );
        let identity = RuntimeBinaryIdentityV1::from_registered_provider(binary)
            .unwrap_or_else(|error| panic!("registered identity for `{provider_id}`: {error}"));

        let original_target = fs::read_link(&target).expect("read dangling provider target");
        let error = install_protocol_binary_target(&source, &target, &artifact_root, &identity)
            .await
            .expect_err("provider must not create a Runtime authority without active ASP");
        assert_eq!(
            fs::read_link(&target).expect("dangling provider target remains unchanged"),
            original_target,
            "{provider_id}"
        );
        assert!(
            error.contains("runtime-active-generation-unavailable"),
            "unexpected `{language_id}` / `{provider_id}` error: {error}"
        );
        assert!(!artifact_root.join("active").exists(), "{provider_id}");

        fs::remove_dir_all(root).expect("remove protocol binary fixture");
    }
}

#[tokio::test]
async fn loop_or_escape_fails_before_lattice_profile_switch() {
    let root = fixture_root("fail-closed");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = root
        .join("home/.local/bin")
        .join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let identity = RuntimeBinaryIdentityV1::asp_bootstrap();
    let first = fixture_source(&root, "source-asp-v1", b"protocol-binary-v1");
    install_protocol_binary_target(&first, &stable_entry, &artifact_root, &identity)
        .await
        .expect("install first Lattice protocol binary");
    commit_pending_runtime_activation(&root).await;

    fs::remove_file(&stable_entry).expect("remove stable entry");
    let escaped = fixture_source(&root, "escaped-asp", b"escaped");
    std::os::unix::fs::symlink(&escaped, &stable_entry).expect("stage escaped stable entry");
    let second = fixture_source(&root, "source-asp-v2", b"protocol-binary-v2");
    let escape_error =
        install_protocol_binary_target(&second, &stable_entry, &artifact_root, &identity)
            .await
            .expect_err("escaped stable entry must fail closed");
    assert!(escape_error.contains("escapes immutable artifact root"));
    assert_eq!(
        fs::read_link(&stable_entry).expect("escaped profile remains unchanged"),
        escaped
    );

    fs::remove_file(&stable_entry).expect("remove escaped stable entry");
    std::os::unix::fs::symlink(&stable_entry, &stable_entry).expect("stage looping stable entry");
    let loop_error =
        install_protocol_binary_target(&second, &stable_entry, &artifact_root, &identity)
            .await
            .expect_err("looping stable entry must fail closed");
    assert!(loop_error.contains("symlink chain loops"));
    assert_eq!(
        fs::read_link(&stable_entry).expect("looping profile remains unchanged"),
        stable_entry
    );

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[tokio::test]
async fn developer_publication_uses_the_immutable_activation_transaction() {
    let root = fixture_root("developer-direct-authority");
    let state_home = root.join("state");
    let runtime_root = state_home.join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let target = root.join("home/.local/bin/asp");
    let checkout = root.join("checkout");
    let source = checkout.join("target/debug/asp");
    fs::create_dir_all(source.parent().expect("developer build directory"))
        .expect("create developer build directory");
    fs::create_dir_all(state_home.join("control/config"))
        .expect("create state-home control config directory");
    fs::write(
        state_home.join("control/config/asp.toml"),
        format!("[dev]\nenabled = true\nroot = {:?}\n", checkout),
    )
    .expect("write developer authority config");
    fs::write(&source, b"developer-asp-v1").expect("write first developer binary");

    let first = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("publish first developer binary");
    assert_eq!(first.status, "published-active-awaiting-health");
    let first_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read first Developer activation")
        .expect("first Developer pending generation");
    assert_eq!(
        first_event.artifact_digest.to_string(),
        first.artifact_digest
    );
    assert!(first_event.artifact_path.starts_with(&artifact_root));
    assert_eq!(
        fs::read(&first_event.artifact_path).unwrap(),
        b"developer-asp-v1"
    );
    assert!(
        !fs::symlink_metadata(&first_event.artifact_path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_ne!(
        fs::canonicalize(&target).ok(),
        fs::canonicalize(&source).ok()
    );
    fs::remove_dir_all(&checkout).expect("clean checkout target");
    assert_eq!(
        fs::read(&first_event.artifact_path).expect("read immutable Developer candidate"),
        b"developer-asp-v1"
    );
    fs::create_dir_all(source.parent().expect("developer build directory"))
        .expect("recreate developer build directory");

    fs::write(&source, b"developer-asp-v2").expect("write second developer binary");
    let second = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("publish second developer binary");
    assert_eq!(second.status, "published-active-awaiting-health");
    let second_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read second Developer activation")
        .expect("second Developer pending generation");
    assert_ne!(
        second_event.publication_nonce,
        first_event.publication_nonce
    );
    assert_ne!(second_event.artifact_digest, first_event.artifact_digest);
    assert_eq!(
        fs::read(&second_event.artifact_path).unwrap(),
        b"developer-asp-v2"
    );
    assert_ne!(
        fs::canonicalize(&target).ok(),
        fs::canonicalize(&source).ok()
    );

    let repeated = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("republish unchanged Developer binary");
    assert_eq!(repeated.status, "published-active-awaiting-health");
    let repeated_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read repeated Developer activation")
        .expect("repeated Developer pending generation");
    assert_eq!(repeated_event.artifact_digest, second_event.artifact_digest);
    assert_ne!(
        repeated_event.publication_nonce,
        second_event.publication_nonce
    );

    fs::remove_dir_all(root).expect("remove developer publication fixture");
}

#[test]
fn qualified_provider_publication_derives_state_home_from_the_stable_launcher() {
    let root = fixture_root("qualified-provider-stable-launcher");
    let state_home = root.join("state");
    let runtime_root = state_home.join("runtime");
    let provider_target = runtime_root.join("bin/asp-rust");
    let resolved =
        qualified_provider_state_home(&provider_target, std::ffi::OsStr::new("asp-rust"))
            .expect("derive State Home from stable provider launcher");

    assert_eq!(resolved, state_home);
    fs::remove_dir_all(root).expect("remove provider publication fixture");
}

#[tokio::test]
async fn release_publication_uses_the_same_immutable_activation_transaction() {
    let root = fixture_root("release-immutable-authority");
    let state_home = root.join("state");
    let runtime_root = state_home.join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let target = root.join("home/.local/bin/asp");
    let source = root.join("release/asp");
    fs::create_dir_all(source.parent().expect("release source parent"))
        .expect("create release source parent");
    fs::write(&source, b"release-asp-v1").expect("write release binary");

    let first = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("publish first Release activation");
    assert_eq!(first.status, "published-active-awaiting-health");
    let first_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read first Release activation")
        .expect("first Release pending generation");
    assert_eq!(
        first_event.artifact_digest.to_string(),
        first.artifact_digest
    );
    assert!(first_event.artifact_path.starts_with(&artifact_root));
    assert_eq!(
        fs::read(&first_event.artifact_path).unwrap(),
        b"release-asp-v1"
    );
    assert_ne!(
        fs::canonicalize(&target).ok(),
        fs::canonicalize(&source).ok()
    );

    let repeated = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("republish unchanged Release binary");
    assert_eq!(repeated.status, "published-active-awaiting-health");
    let repeated_event = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read repeated Release activation")
        .expect("repeated Release pending generation");
    assert_eq!(repeated_event.artifact_digest, first_event.artifact_digest);
    assert_ne!(
        repeated_event.publication_nonce,
        first_event.publication_nonce
    );

    fs::remove_dir_all(root).expect("remove release publication fixture");
}

#[tokio::test]
async fn explicit_candidate_source_bytes_replace_existing_canonical_binary() {
    let root = fixture_root("explicit-candidate-source");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let target = root
        .join("home/.local/bin")
        .join(SEMANTIC_AGENT_PROTOCOL_BIN);
    fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");

    let old_source = fixture_source(&root, "old-source-asp", b"old-asp");
    let candidate = fixture_source(&root, "candidate-asp", b"candidate-asp");
    let old = install_protocol_binary_target(
        &old_source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("publish old canonical binary");
    commit_pending_runtime_activation(&root).await;

    let plan = ProtocolBinaryInstallPlan {
        current_exe: old_source,
        explicit_candidate_source: Some(candidate),
        target: target.clone(),
        artifact_root: artifact_root.clone(),
        binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
    };

    let installed = ensure_protocol_binary_installed(&plan)
        .await
        .expect("publish explicit candidate source");
    commit_pending_runtime_activation(&root).await;
    assert_ne!(installed.artifact_digest, old.artifact_digest);
    assert_eq!(
        fs::read(&target).expect("read canonical target"),
        b"candidate-asp"
    );

    fs::remove_dir_all(root).expect("remove explicit candidate fixture");
}
