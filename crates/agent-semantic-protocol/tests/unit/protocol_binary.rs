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
    let artifact_root = root.join("runtime/artifacts");
    std::fs::write(&source, b"asp artifact one").expect("write source");

    install_protocol_binary_target(&source, &target, &artifact_root)
        .expect("install protocol binary");
    install_protocol_binary_target(&source, &secondary_target, &artifact_root)
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
            .contains("/runtime/artifacts/blake3-256/"),
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
    install_protocol_binary_target(&source, &target, &artifact_root)
        .expect("replace public target");
    install_protocol_binary_target(&source, &secondary_target, &artifact_root)
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
fn unrelated_path_asp_is_ignored_by_canonical_target_selection() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-unrelated-path-gate-{}-{nonce}",
        std::process::id()
    ));
    let ambient_dir = root.join("ambient");
    std::fs::create_dir_all(&ambient_dir).expect("create ambient dir");
    let ambient_asp = ambient_dir.join(super::SEMANTIC_AGENT_PROTOCOL_BIN);
    std::fs::write(&ambient_asp, b"ambient-sentinel").expect("write ambient asp");

    let artifact_root = root.join("runtime/artifacts");
    let target = super::resolve_protocol_binary_install_target(None, &artifact_root)
        .expect("resolve canonical runtime target");
    assert_eq!(
        target,
        root.join("runtime/bin")
            .join(super::SEMANTIC_AGENT_PROTOCOL_BIN)
    );
    assert_eq!(
        std::fs::read(&ambient_asp).expect("read ambient sentinel"),
        b"ambient-sentinel"
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn concurrent_publish_uses_per_attempt_stage_paths() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-concurrent-publish-{}-{nonce}",
        std::process::id()
    ));
    let source = root.join("source/asp");
    let target = root.join("bin/asp");
    let artifact_root = root.join("runtime/artifacts");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source dir");
    std::fs::write(&source, b"concurrent asp artifact").expect("write source");

    std::thread::scope(|scope| {
        let attempts = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    super::install_protocol_binary_target(&source, &target, &artifact_root)
                })
            })
            .collect::<Vec<_>>();
        for attempt in attempts {
            attempt
                .join()
                .expect("publisher thread")
                .expect("publish protocol binary");
        }
    });

    assert_eq!(
        super::protocol_binary_artifact_digest(&target),
        super::protocol_binary_artifact_digest(&source)
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

    let target =
        super::resolve_protocol_binary_install_target(Some(&explicit_bin), &root.join("artifacts"))
            .expect("resolve explicit target");

    assert_eq!(
        target,
        explicit_bin.join(super::SEMANTIC_AGENT_PROTOCOL_BIN)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn install_plan_capture_rejects_non_asp_process_identity() {
    let error = super::ProtocolBinaryInstallPlan::capture(
        std::env::temp_dir().join("asp-install-plan-rejected/runtime/artifacts"),
    )
    .expect_err("unit test executable must not be accepted as the ASP install source");
    assert!(error.contains("semantic hook setup must run through `asp`"));
}

#[cfg(unix)]
#[test]
fn non_login_shell_probe_uses_cold_host_path() {
    use std::os::unix::fs::PermissionsExt;

    let root = protocol_binary_probe_root("found");
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).expect("create probe bin");
    let asp = bin.join(super::SEMANTIC_AGENT_PROTOCOL_BIN);
    std::fs::write(&asp, b"#!/bin/sh\nexit 0\n").expect("write probe asp");
    let mut permissions = std::fs::metadata(&asp)
        .expect("inspect probe asp")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&asp, permissions).expect("chmod probe asp");

    let probe = super::probe_protocol_binary_in_non_login_shell(
        std::path::Path::new("/bin/sh"),
        bin.as_os_str(),
    );

    assert_eq!(
        probe,
        super::ProtocolBinaryShellProbe {
            shell: std::path::PathBuf::from("/bin/sh"),
            path: Some(asp),
            status: "found",
        }
    );
    std::fs::remove_dir_all(root).expect("cleanup probe root");
}

#[cfg(unix)]
#[test]
fn non_login_shell_probe_reports_missing_without_terminal_path() {
    let root = protocol_binary_probe_root("missing");

    let probe = super::probe_protocol_binary_in_non_login_shell(
        std::path::Path::new("/bin/sh"),
        root.as_os_str(),
    );

    assert_eq!(
        probe,
        super::ProtocolBinaryShellProbe {
            shell: std::path::PathBuf::from("/bin/sh"),
            path: None,
            status: "missing",
        }
    );
    std::fs::remove_dir_all(root).expect("cleanup probe root");
}

#[cfg(unix)]
fn protocol_binary_probe_root(case: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-shell-probe-{case}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create probe root");
    root
}
