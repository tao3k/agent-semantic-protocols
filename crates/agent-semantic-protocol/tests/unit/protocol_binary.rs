use super::{install_protocol_binary_target, protocol_binary_artifact_digest};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[test]
fn installed_binary_is_blake3_addressed_and_public_target_is_constant_time() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-blake3-artifact-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("source")).expect("create source dir");
    std::fs::create_dir_all(root.join("bin")).expect("create bin dir");
    std::fs::create_dir_all(root.join("bin-secondary")).expect("create secondary bin dir");
    let source = root.join("source/asp");
    let target = root.join("bin/asp");
    let secondary_target = root.join("bin-secondary/asp");
    std::fs::write(&source, b"asp artifact one").expect("write source");

    install_protocol_binary_target(&source, &target).expect("install protocol binary");
    install_protocol_binary_target(&source, &secondary_target)
        .expect("install secondary protocol binary");
    let source_digest = protocol_binary_artifact_digest(&source).expect("source identity");
    let target_digest = protocol_binary_artifact_digest(&target).expect("target identity");
    assert_eq!(source_digest, target_digest);
    let first_artifact = std::fs::canonicalize(&target).expect("first artifact");
    assert_eq!(
        first_artifact,
        std::fs::canonicalize(&secondary_target).expect("secondary artifact")
    );
    assert!(
        first_artifact
            .to_string_lossy()
            .contains("/.asp-artifacts/blake3-256/"),
        "{}",
        first_artifact.display()
    );

    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = Instant::now();
        assert_eq!(
            protocol_binary_artifact_digest(&target),
            Some(target_digest.clone())
        );
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    println!(
        "[protocol-binary-identity-perf] samples={} p95Micros={} budgetMicros=1000",
        samples.len(),
        p95.as_micros()
    );
    assert!(
        p95 < std::time::Duration::from_millis(1),
        "digest-addressed identity p95 exceeded 1ms: {p95:?}"
    );

    std::fs::write(&source, b"asp artifact two with different bytes")
        .expect("replace source binary");
    let changed_digest = protocol_binary_artifact_digest(&source).expect("changed source digest");
    assert_ne!(changed_digest, target_digest);
    assert_eq!(
        protocol_binary_artifact_digest(&target),
        Some(target_digest)
    );
    install_protocol_binary_target(&source, &target).expect("replace public target");
    install_protocol_binary_target(&source, &secondary_target)
        .expect("replace secondary public target");
    let second_artifact = std::fs::canonicalize(&target).expect("second artifact");
    assert_eq!(
        second_artifact,
        std::fs::canonicalize(&secondary_target).expect("secondary replacement")
    );
    assert_ne!(first_artifact, second_artifact);
    assert!(
        first_artifact.is_file(),
        "first artifact must remain immutable"
    );
    assert_eq!(
        protocol_binary_artifact_digest(&target),
        Some(changed_digest)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn unrelated_path_asp_is_never_selected_or_modified() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-unrelated-path-gate-{}-{nonce}",
        std::process::id()
    ));
    let current_dir = root.join("current");
    let ambient_dir = root.join("ambient");
    std::fs::create_dir_all(&current_dir).expect("create current dir");
    std::fs::create_dir_all(&ambient_dir).expect("create ambient dir");
    let current_exe = current_dir.join(super::SEMANTIC_AGENT_PROTOCOL_BIN);
    let ambient_asp = ambient_dir.join(super::SEMANTIC_AGENT_PROTOCOL_BIN);
    std::fs::write(&current_exe, b"current").expect("write current asp");
    std::fs::write(&ambient_asp, b"ambient-sentinel").expect("write ambient asp");

    let error = super::resolve_protocol_binary_install_target(&current_exe, None, &[ambient_dir])
        .expect_err("unrelated PATH asp must fail closed");

    assert!(error.contains("refusing to update unrelated PATH binary"));
    assert_eq!(
        std::fs::read(&ambient_asp).expect("read ambient sentinel"),
        b"ambient-sentinel"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn explicit_bin_root_selects_exactly_one_target() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-explicit-root-gate-{}-{nonce}",
        std::process::id()
    ));
    let current_exe = root.join(super::SEMANTIC_AGENT_PROTOCOL_BIN);
    let explicit_bin = root.join("explicit-bin");
    let ambient_bin = root.join("ambient-bin");
    std::fs::create_dir_all(&explicit_bin).expect("create explicit bin");
    std::fs::create_dir_all(&ambient_bin).expect("create ambient bin");
    std::fs::write(&current_exe, b"current").expect("write current asp");
    std::fs::write(
        ambient_bin.join(super::SEMANTIC_AGENT_PROTOCOL_BIN),
        b"ambient-sentinel",
    )
    .expect("write ambient asp");

    let target = super::resolve_protocol_binary_install_target(
        &current_exe,
        Some(&explicit_bin),
        &[ambient_bin],
    )
    .expect("resolve explicit target");

    assert_eq!(
        target,
        explicit_bin.join(super::SEMANTIC_AGENT_PROTOCOL_BIN)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn install_plan_capture_rejects_non_asp_process_identity() {
    let error = super::ProtocolBinaryInstallPlan::capture()
        .expect_err("unit test executable must not be accepted as the ASP install source");
    assert!(error.contains("semantic hook setup must run through `asp`"));
}
