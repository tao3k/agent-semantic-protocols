use super::{
    ProtocolBinaryInstallPlan, SEMANTIC_AGENT_PROTOCOL_BIN, ensure_protocol_binary_installed,
    install_protocol_binary_target, next_protocol_binary_publish_sequence,
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

fn fixture_source(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let source = root.join(name);
    fs::write(&source, bytes).expect("write protocol binary fixture");
    source
}

use super::RuntimeBinaryIdentityV1;

#[test]
fn latest_aliases_and_multi_binary_switches_are_isolated() {
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
        ensure_protocol_binary_installed(&plan).expect("install immutable protocol binary");
    let artifact = artifact_root
        .join("blake3-256")
        .join(&installed.artifact_digest)
        .join(SEMANTIC_AGENT_PROTOCOL_BIN);

    assert_eq!(
        fs::read_link(&installed.latest).expect("read latest link"),
        PathBuf::from("..")
            .join(&installed.artifact_digest)
            .join(SEMANTIC_AGENT_PROTOCOL_BIN)
    );
    assert_eq!(
        fs::read_link(&stable_entry).expect("read stable entry"),
        PathBuf::from("../artifacts/blake3-256/latest").join(SEMANTIC_AGENT_PROTOCOL_BIN)
    );
    assert_eq!(
        fs::canonicalize(&stable_entry).expect("resolve stable entry"),
        fs::canonicalize(&artifact).expect("resolve immutable artifact")
    );
    assert_eq!(
        fs::canonicalize(&alias).expect("resolve alias"),
        fs::canonicalize(&artifact).expect("resolve immutable artifact")
    );

    let asp_latest_target = fs::read_link(&installed.latest).expect("read asp latest link");
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
    let harness_latest_target =
        fs::read_link(&harness_install.latest).expect("read harness latest link");
    assert_eq!(
        harness_latest_target,
        PathBuf::from("..")
            .join(&harness_install.artifact_digest)
            .join(harness_name)
    );
    assert_eq!(
        fs::read_link(&harness_stable).expect("read harness stable entry"),
        PathBuf::from("../artifacts/blake3-256/latest").join(harness_name)
    );
    assert_eq!(
        fs::read_link(&installed.latest).expect("asp latest remains isolated"),
        asp_latest_target
    );

    let second_asp = fixture_source(&root, "source-asp-v2", b"protocol-binary-v2");
    let second_asp_install = install_protocol_binary_target(
        &second_asp,
        &stable_entry,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("switch asp latest independently");
    assert_ne!(
        fs::read_link(&second_asp_install.latest).expect("read switched asp latest"),
        asp_latest_target
    );
    assert_eq!(
        fs::read_link(&harness_install.latest).expect("harness latest remains isolated"),
        harness_latest_target
    );

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
fn loop_or_escape_fails_before_latest_switch() {
    let root = fixture_root("fail-closed");
    let runtime = root.join("runtime");
    let artifact_root = runtime.join("artifacts");
    let stable_entry = runtime.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let identity = RuntimeBinaryIdentityV1::asp_bootstrap();
    let first = fixture_source(&root, "source-asp-v1", b"protocol-binary-v1");
    install_protocol_binary_target(&first, &stable_entry, &artifact_root, &identity)
        .expect("install first immutable protocol binary");
    let latest = artifact_root
        .join("blake3-256")
        .join("latest")
        .join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let first_latest = fs::read_link(&latest).expect("read first latest link");

    fs::remove_file(&stable_entry).expect("remove stable entry");
    let escaped = fixture_source(&root, "escaped-asp", b"escaped");
    std::os::unix::fs::symlink(&escaped, &stable_entry).expect("stage escaped stable entry");
    let second = fixture_source(&root, "source-asp-v2", b"protocol-binary-v2");
    let escape_error =
        install_protocol_binary_target(&second, &stable_entry, &artifact_root, &identity)
            .expect_err("escaped stable entry must fail closed");
    assert!(escape_error.contains("escapes immutable artifact root"));
    assert_eq!(
        fs::read_link(&latest).expect("latest remains after escape"),
        first_latest
    );

    fs::remove_file(&stable_entry).expect("remove escaped stable entry");
    std::os::unix::fs::symlink(&stable_entry, &stable_entry).expect("stage looping stable entry");
    let loop_error =
        install_protocol_binary_target(&second, &stable_entry, &artifact_root, &identity)
            .expect_err("looping stable entry must fail closed");
    assert!(loop_error.contains("symlink chain loops"));
    assert_eq!(
        fs::read_link(&latest).expect("latest remains after loop"),
        first_latest
    );

    fs::remove_dir_all(&root).expect("remove protocol binary fixture");
}
