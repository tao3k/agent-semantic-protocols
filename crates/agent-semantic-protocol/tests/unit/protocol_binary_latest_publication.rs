use super::{
    ProtocolBinaryInstallPlan, SEMANTIC_AGENT_PROTOCOL_BIN, ensure_protocol_binary_installed,
    install_protocol_binary_alias, install_protocol_binary_target,
    next_protocol_binary_publish_sequence, protocol_binary_artifact_path_digest,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

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
    let target = root.join("runtime/bin/asp");
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
                .artifact_digest
                .strip_prefix("blake3-256:")
                .expect("canonical installed digest")
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
        agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(
            state_home,
        )
        .await
        .expect("read pending Runtime activation")
        .expect("pending Runtime activation event");
    agent_semantic_artifacts::runtime_artifact_publication::commit_runtime_artifact_activation(
        state_home, &event, None,
    )
    .await
    .expect("commit pending Runtime activation");
}

use super::RuntimeBinaryIdentityV1;

#[tokio::test]
async fn lattice_profile_slots_and_multi_binary_switches_are_isolated() {
    let root = fixture_root("latest-aliases");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let alias = root.join("path-bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let source = fixture_source(&root, "source-asp", b"protocol-binary-v1");
    let plan = ProtocolBinaryInstallPlan {
        current_exe: source,
        explicit_candidate_source: None,
        target: stable_entry.clone(),
        artifact_root: artifact_root.clone(),
        managed_path_aliases: vec![alias.clone()],
        binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
    };

    let installed = ensure_protocol_binary_installed(&plan)
        .await
        .expect("install Lattice protocol binary");
    commit_pending_runtime_activation(&root).await;
    install_protocol_binary_alias(&alias, &stable_entry, &artifact_root)
        .expect("publish PATH alias after resident activation");
    assert_eq!(installed.path, stable_entry);
    assert!(
        fs::symlink_metadata(&stable_entry)
            .expect("inspect stable entry")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read(&stable_entry).expect("read stable entry"),
        b"protocol-binary-v1"
    );
    assert_eq!(
        fs::canonicalize(&alias).expect("resolve alias"),
        fs::canonicalize(&stable_entry).expect("resolve stable entry")
    );

    let asp_digest = installed.artifact_digest.clone();
    assert!(
        artifact_root
            .join("blake3-256")
            .join(
                asp_digest
                    .strip_prefix("blake3-256:")
                    .expect("canonical ASP digest"),
            )
            .join(SEMANTIC_AGENT_PROTOCOL_BIN)
            .is_file()
    );
    let harness_name = "rs-harness";
    let harness_stable = runtime.join("bin").join(harness_name);
    let harness_source = fixture_source(&root, "source-rs-harness", b"rust-harness-v1");
    let harness_identity = RuntimeBinaryIdentityV1::from_registered_provider(harness_name)
        .expect("registered harness binary identity");
    let harness_install = install_protocol_binary_target(
        &harness_source,
        &harness_stable,
        &artifact_root,
        &harness_identity,
    )
    .await
    .expect("install immutable harness binary");
    commit_pending_runtime_activation(&root).await;
    assert_eq!(harness_install.path, harness_stable);
    assert_eq!(
        fs::read(&harness_stable).expect("read harness stable entry"),
        b"rust-harness-v1"
    );
    assert_eq!(installed.artifact_digest, asp_digest);

    let second_asp = fixture_source(&root, "source-asp-v2", b"protocol-binary-v2");
    let second_asp_install = install_protocol_binary_target(
        &second_asp,
        &stable_entry,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .await
    .expect("switch asp latest independently");
    commit_pending_runtime_activation(&root).await;
    assert_ne!(second_asp_install.artifact_digest, asp_digest);
    assert_eq!(
        fs::read(&stable_entry).expect("read switched ASP profile"),
        b"protocol-binary-v2"
    );
    assert_eq!(
        fs::read(&harness_stable).expect("harness profile remains isolated"),
        b"rust-harness-v1"
    );
    assert!(artifact_root.join("blake3-256").exists());

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[tokio::test]
async fn runtime_publication_rejects_target_name_inference_and_path_shaped_identities() {
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("../rs-harness").is_err());
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("bin/rs-harness").is_err());

    let root = fixture_root("declared-binary-mismatch");
    let artifact_root = root.join("runtime/artifacts");
    let source = fixture_source(&root, "source-rs-harness", b"rust-harness-v1");
    let wrong_target = root.join("runtime/bin/not-rs-harness");
    let identity = RuntimeBinaryIdentityV1::from_registered_provider("rs-harness")
        .expect("registered harness binary identity");

    let error = install_protocol_binary_target(&source, &wrong_target, &artifact_root, &identity)
        .await
        .expect_err("target filename must not override ProviderRegistry identity");
    assert!(error.contains("does not match declared binary identity"));

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[tokio::test]
async fn registered_scheme_and_python_dangling_entries_are_atomically_republished() {
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

        let installed = install_protocol_binary_target(&source, &target, &artifact_root, &identity)
            .await
            .unwrap_or_else(|error| panic!("publish `{language_id}` / `{provider_id}`: {error}"));
        assert_eq!(
            installed.status, "published-activation-pending",
            "{provider_id}"
        );
        commit_pending_runtime_activation(&root).await;
        assert_eq!(installed.path, target, "{provider_id}");
        assert!(
            fs::symlink_metadata(&target)
                .expect("inspect republished provider")
                .file_type()
                .is_symlink(),
            "{provider_id}"
        );
        assert_eq!(
            fs::read(&target).expect("read republished provider"),
            provider_id.as_bytes(),
            "{provider_id}"
        );
        assert!(
            artifact_root
                .join("blake3-256")
                .join(
                    installed
                        .artifact_digest
                        .strip_prefix("blake3-256:")
                        .expect("canonical installed digest"),
                )
                .join(binary)
                .is_file()
        );

        fs::remove_dir_all(root).expect("remove protocol binary fixture");
    }
}

#[tokio::test]
async fn loop_or_escape_fails_before_lattice_profile_switch() {
    let root = fixture_root("fail-closed");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
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
async fn lattice_reconciliation_retains_only_reachable_digest_generations() {
    let root = fixture_root("retention");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let asp_target = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let harness_name = "rs-harness";
    let harness_target = runtime.join("bin").join(harness_name);
    let asp_identity = RuntimeBinaryIdentityV1::asp_bootstrap();
    let harness_identity =
        RuntimeBinaryIdentityV1::from_registered_provider(harness_name).expect("harness identity");

    for version in 0..4 {
        let source = fixture_source(
            &root,
            &format!("source-asp-{version}"),
            format!("protocol-binary-{version}").as_bytes(),
        );
        install_protocol_binary_target(&source, &asp_target, &artifact_root, &asp_identity)
            .await
            .expect("publish ASP generation");
        commit_pending_runtime_activation(&root).await;
        let source = fixture_source(
            &root,
            &format!("source-harness-{version}"),
            format!("harness-binary-{version}").as_bytes(),
        );
        install_protocol_binary_target(&source, &harness_target, &artifact_root, &harness_identity)
            .await
            .expect("publish harness generation");
        commit_pending_runtime_activation(&root).await;
    }

    let receipt =
        agent_semantic_artifacts::runtime_artifact_retention::prune_unreachable_runtime_artifacts(
            &artifact_root,
        )
        .await
        .expect("prune artifact history");
    assert_eq!(receipt.scanned_generation_count, 8);
    assert_eq!(receipt.retained_generation_count, 4);
    assert_eq!(receipt.removed_generation_count, 4);
    assert_eq!(receipt.ignored_entry_count, 0);
    assert!(receipt.reclaimed_bytes > 0);
    assert_eq!(receipt.protected_digests.len(), 4);
    assert!(fs::canonicalize(&asp_target).is_ok());
    assert!(fs::canonicalize(&harness_target).is_ok());
    assert!(artifact_root.join("blake3-256").is_dir());
    assert!(artifact_root.join("retention-receipt.json").is_file());

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[tokio::test]
async fn developer_publication_uses_the_immutable_activation_transaction() {
    let root = fixture_root("developer-direct-authority");
    let state_home = root.join("state");
    let runtime_root = state_home.join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let target = runtime_root.join("bin/asp");
    let checkout = root.join("checkout");
    let source = checkout.join("target/debug/asp");
    fs::create_dir_all(source.parent().expect("developer build directory"))
        .expect("create developer build directory");
    fs::create_dir_all(&state_home).expect("create state home");
    fs::write(
        state_home.join("asp.toml"),
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
    assert_eq!(first.status, "published-activation-pending");
    let first_event = agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(&state_home)
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
    assert_eq!(second.status, "published-activation-pending");
    let second_event = agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read second Developer activation")
        .expect("second Developer pending generation");
    assert!(second_event.activation_generation > first_event.activation_generation);
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
    assert_eq!(repeated.status, "published-activation-pending");
    let repeated_event = agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read repeated Developer activation")
        .expect("repeated Developer pending generation");
    assert_eq!(repeated_event.artifact_digest, second_event.artifact_digest);
    assert!(repeated_event.activation_generation > second_event.activation_generation);

    fs::remove_dir_all(root).expect("remove developer publication fixture");
}

#[tokio::test]
async fn release_publication_uses_the_same_immutable_activation_transaction() {
    let root = fixture_root("release-immutable-authority");
    let state_home = root.join("state");
    let runtime_root = state_home.join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let target = runtime_root.join("bin/asp");
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
    assert_eq!(first.status, "published-activation-pending");
    let first_event = agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(&state_home)
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
    assert_eq!(repeated.status, "published-activation-pending");
    let repeated_event = agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read repeated Release activation")
        .expect("repeated Release pending generation");
    assert_eq!(repeated_event.artifact_digest, first_event.artifact_digest);
    assert!(repeated_event.activation_generation > first_event.activation_generation);

    fs::remove_dir_all(root).expect("remove release publication fixture");
}

#[tokio::test]
async fn explicit_candidate_source_bytes_replace_existing_canonical_binary() {
    let root = fixture_root("explicit-candidate-source");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let target = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
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
        managed_path_aliases: vec![],
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
