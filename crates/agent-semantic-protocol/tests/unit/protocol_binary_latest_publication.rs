use super::{
    ProtocolBinaryInstallPlan, SEMANTIC_AGENT_PROTOCOL_BIN,
    canonical_protocol_binary_artifact_digest, ensure_protocol_binary_installed,
    install_protocol_binary_target, managed_protocol_binary_path_aliases,
    next_protocol_binary_publish_sequence, prune_runtime_binary_artifacts,
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
async fn canonical_control_digest_is_a_sub_millisecond_path_identity_lookup() {
    let root = fixture_root("control-digest-fast-path");
    let artifact_root = root.join("runtime/artifacts");
    let target = root.join("runtime/bin/asp");
    let source = fixture_source(&root, "source-asp", b"control-digest-fixture");
    let installed = install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("publish digest-addressed ASP artifact");

    let mut samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = std::time::Instant::now();
        let digest = canonical_protocol_binary_artifact_digest(&target)
            .await
            .expect("read canonical artifact path identity");
        samples.push(started.elapsed());
        assert_eq!(digest, installed.artifact_digest);
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-binary-control-fast-path] requests=10000 p99Nanos={} binaryByteReads=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "canonical runtime digest path lookup p99 must remain sub-millisecond, observed {p99:?}"
    );

    fs::remove_dir_all(root).expect("remove protocol binary fixture");
}

fn fixture_source(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let source = root.join(name);
    fs::write(&source, bytes).expect("write protocol binary fixture");
    source
}

#[test]
fn install_plan_always_publishes_the_state_home_bin_alias() {
    let root = fixture_root("state-home-bin-alias");
    let artifact_root = root.join("runtime/artifacts");
    let stable_entry = root.join("runtime/bin/asp");
    let state_home_alias = root.join(".bin/asp");
    let aliases = managed_protocol_binary_path_aliases(&artifact_root, &stable_entry, &[])
        .expect("derive managed aliases");
    assert_eq!(aliases, vec![state_home_alias.clone()]);

    let source = fixture_source(&root, "source-asp", b"protocol-binary");
    let plan = ProtocolBinaryInstallPlan {
        current_exe: source,
        target: stable_entry.clone(),
        artifact_root,
        managed_path_aliases: aliases,
        binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
    };
    ensure_protocol_binary_installed(&plan).expect("publish canonical binary and State Home alias");
    assert_eq!(
        fs::read_link(&state_home_alias).expect("State Home alias"),
        stable_entry
    );
    fs::remove_dir_all(root).expect("remove protocol binary fixture");
}

use super::RuntimeBinaryIdentityV1;
use agent_semantic_hook::registered_provider_binaries_v1;

#[test]
fn lattice_profile_slots_and_multi_binary_switches_are_isolated() {
    let root = fixture_root("latest-aliases");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let alias = root.join("path-bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let source = fixture_source(&root, "source-asp", b"protocol-binary-v1");
    let plan = ProtocolBinaryInstallPlan {
        current_exe: source,
        target: stable_entry.clone(),
        artifact_root: artifact_root.clone(),
        managed_path_aliases: vec![alias.clone()],
        binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
    };

    let installed =
        ensure_protocol_binary_installed(&plan).expect("install Lattice protocol binary");
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
            .join(&asp_digest)
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
    .expect("install immutable harness binary");
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
    .expect("switch asp latest independently");
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

#[test]
fn runtime_publication_rejects_target_name_inference_and_path_shaped_identities() {
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("../rs-harness").is_err());
    assert!(RuntimeBinaryIdentityV1::from_registered_provider("bin/rs-harness").is_err());

    let root = fixture_root("declared-binary-mismatch");
    let artifact_root = root.join("runtime/artifacts");
    let source = fixture_source(&root, "source-rs-harness", b"rust-harness-v1");
    let wrong_target = root.join("runtime/bin/not-rs-harness");
    let identity = RuntimeBinaryIdentityV1::from_registered_provider("rs-harness")
        .expect("registered harness binary identity");

    let error = install_protocol_binary_target(&source, &wrong_target, &artifact_root, &identity)
        .expect_err("target filename must not override ProviderRegistry identity");
    assert!(error.contains("does not match declared binary identity"));

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[test]
fn registered_scheme_and_python_dangling_entries_are_atomically_republished() {
    let registrations = registered_provider_binaries_v1();
    for language_id in ["gerbil-scheme", "python"] {
        let registration = registrations
            .iter()
            .find(|registration| registration.language_id().as_str() == language_id)
            .unwrap_or_else(|| panic!("registered language `{language_id}`"));
        let provider_id = registration.provider_id().as_str();
        let root = fixture_root(provider_id);
        let artifact_root = root.join("runtime/artifacts");
        let target = root.join("runtime/bin").join(registration.binary());
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
        let identity = RuntimeBinaryIdentityV1::from_registered_provider(registration.binary())
            .unwrap_or_else(|error| panic!("registered identity for `{provider_id}`: {error}"));

        let installed = install_protocol_binary_target(&source, &target, &artifact_root, &identity)
            .unwrap_or_else(|error| panic!("publish `{language_id}` / `{provider_id}`: {error}"));
        assert_eq!(installed.status, "updated", "{provider_id}");
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
                .join(&installed.artifact_digest)
                .join(registration.binary())
                .is_file()
        );

        fs::remove_dir_all(root).expect("remove protocol binary fixture");
    }
}

#[test]
fn loop_or_escape_fails_before_lattice_profile_switch() {
    let root = fixture_root("fail-closed");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let identity = RuntimeBinaryIdentityV1::asp_bootstrap();
    let first = fixture_source(&root, "source-asp-v1", b"protocol-binary-v1");
    install_protocol_binary_target(&first, &stable_entry, &artifact_root, &identity)
        .expect("install first Lattice protocol binary");

    fs::remove_file(&stable_entry).expect("remove stable entry");
    let escaped = fixture_source(&root, "escaped-asp", b"escaped");
    std::os::unix::fs::symlink(&escaped, &stable_entry).expect("stage escaped stable entry");
    let second = fixture_source(&root, "source-asp-v2", b"protocol-binary-v2");
    let escape_error =
        install_protocol_binary_target(&second, &stable_entry, &artifact_root, &identity)
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
            .expect_err("looping stable entry must fail closed");
    assert!(loop_error.contains("symlink chain loops"));
    assert_eq!(
        fs::read_link(&stable_entry).expect("looping profile remains unchanged"),
        stable_entry
    );

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}

#[test]
fn lattice_reconciliation_retains_only_reachable_digest_generations() {
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
            .expect("publish ASP generation");
        let source = fixture_source(
            &root,
            &format!("source-harness-{version}"),
            format!("harness-binary-{version}").as_bytes(),
        );
        install_protocol_binary_target(&source, &harness_target, &artifact_root, &harness_identity)
            .expect("publish harness generation");
    }

    let receipt = prune_runtime_binary_artifacts(&artifact_root).expect("prune artifact history");
    assert_eq!(receipt.scanned_generation_count, 8);
    assert_eq!(receipt.retained_generation_count, 2);
    assert_eq!(receipt.removed_generation_count, 6);
    assert_eq!(receipt.ignored_entry_count, 0);
    assert_eq!(receipt.reclaimed_bytes, 99);
    assert_eq!(receipt.protected_digests.len(), 2);
    assert!(fs::canonicalize(&asp_target).is_ok());
    assert!(fs::canonicalize(&harness_target).is_ok());
    assert!(artifact_root.join("blake3-256").is_dir());
    assert!(artifact_root.join("retention-receipt.v1.json").is_file());

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}
