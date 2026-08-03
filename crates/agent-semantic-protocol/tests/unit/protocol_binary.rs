use super::install_protocol_binary_target;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RuntimeBinaryIdentityV1;

#[test]
fn installed_binary_is_a_lattice_current_profile_without_digest_history() {
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

    install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("install protocol binary");
    install_protocol_binary_target(
        &source,
        &secondary_target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("install secondary protocol binary");
    assert_eq!(
        std::fs::read(&target).expect("read target"),
        b"asp artifact one"
    );
    assert_eq!(
        std::fs::read(&secondary_target).expect("read secondary target"),
        b"asp artifact one"
    );
    assert!(
        std::fs::symlink_metadata(&target)
            .expect("target metadata")
            .file_type()
            .is_file(),
        "the Lattice current profile must be a regular file"
    );
    assert!(
        std::fs::symlink_metadata(&secondary_target)
            .expect("secondary target metadata")
            .file_type()
            .is_file(),
        "each Lattice profile slot must be a regular file"
    );
    assert!(
        !artifact_root.join("blake3-256").exists(),
        "install must not create digest-addressed runtime history"
    );

    std::fs::write(&source, b"asp artifact two with different bytes")
        .expect("replace source binary");
    assert_eq!(
        std::fs::read(&target).expect("read unchanged target"),
        b"asp artifact one"
    );
    install_protocol_binary_target(
        &source,
        &target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("replace public target");
    assert_eq!(
        std::fs::read(&target).expect("read replaced target"),
        b"asp artifact two with different bytes"
    );
    assert_eq!(
        std::fs::read(&secondary_target).expect("read isolated secondary target"),
        b"asp artifact one",
        "publishing one Lattice profile must not mutate another profile slot"
    );
    install_protocol_binary_target(
        &source,
        &secondary_target,
        &artifact_root,
        &RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("replace secondary public target");
    assert_eq!(
        std::fs::read(&secondary_target).expect("read replaced secondary target"),
        b"asp artifact two with different bytes"
    );
    assert!(!artifact_root.join("blake3-256").exists());
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

#[cfg(unix)]
#[test]
fn runtime_health_establishes_the_user_path_symlink() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-user-path-alias-{}-{nonce}",
        std::process::id()
    ));
    let protocol_home = root.join("state-home");
    let source = root.join("source/asp");
    let canonical_target = protocol_home.join("runtime/bin/asp");
    let artifact_root = protocol_home.join("runtime/artifacts");
    let user_path_alias = root.join("home/.local/bin/asp");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    std::fs::write(&source, b"runtime asp").expect("write source");
    super::install_protocol_binary_target(
        &source,
        &canonical_target,
        &artifact_root,
        &super::RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("install canonical runtime target");

    super::ensure_runtime_protocol_binary_alias(&protocol_home, &user_path_alias)
        .expect("establish runtime PATH alias");

    assert_eq!(
        std::fs::read_link(&user_path_alias).expect("read runtime PATH alias"),
        canonical_target
    );
    assert_eq!(
        std::fs::canonicalize(&user_path_alias).expect("resolve runtime PATH alias"),
        std::fs::canonicalize(protocol_home.join("runtime/bin/asp"))
            .expect("resolve canonical runtime target")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn runtime_health_refuses_an_unmanaged_user_path_entry() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-user-path-refusal-{}-{nonce}",
        std::process::id()
    ));
    let protocol_home = root.join("state-home");
    let source = root.join("source/asp");
    let canonical_target = protocol_home.join("runtime/bin/asp");
    let artifact_root = protocol_home.join("runtime/artifacts");
    let user_path_alias = root.join("home/.local/bin/asp");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    std::fs::create_dir_all(user_path_alias.parent().expect("alias parent"))
        .expect("create alias parent");
    std::fs::write(&source, b"runtime asp").expect("write source");
    std::fs::write(&user_path_alias, b"unmanaged asp").expect("write unmanaged entry");
    super::install_protocol_binary_target(
        &source,
        &canonical_target,
        &artifact_root,
        &super::RuntimeBinaryIdentityV1::asp_bootstrap(),
    )
    .expect("install canonical runtime target");

    let error = super::ensure_runtime_protocol_binary_alias(&protocol_home, &user_path_alias)
        .expect_err("unmanaged runtime PATH entry must fail closed");

    assert!(error.contains("refusing to replace unmanaged ASP runtime PATH entry"));
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}

#[test]
fn missing_artifact_root_is_not_a_digest_addressed_binary() {
    let root = std::env::temp_dir().join(format!(
        "asp-binary-missing-artifact-root-{}",
        std::process::id()
    ));
    let identity = root.join("ambient/asp");
    let artifact_root = root.join("runtime/artifacts");

    assert!(
        !super::is_digest_addressed_protocol_binary(&identity, &artifact_root)
            .expect("a not-yet-materialized artifact root is not an install failure")
    );
}

#[test]
fn canonical_runtime_artifact_identity_is_derived_without_reading_binary_bytes() {
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let canonical = std::path::Path::new("/state/runtime/artifacts/blake3-256")
        .join(digest)
        .join("asp");

    assert_eq!(
        super::protocol_binary_digest_from_canonical_artifact_path(&canonical).as_deref(),
        Some(digest)
    );
    assert!(
        super::protocol_binary_digest_from_canonical_artifact_path(std::path::Path::new(
            "/tmp/target/debug/asp"
        ))
        .is_none(),
        "non-canonical binaries must fail closed instead of triggering query-time byte hashing"
    );
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
                    super::install_protocol_binary_target(
                        &source,
                        &target,
                        &artifact_root,
                        &super::RuntimeBinaryIdentityV1::asp_bootstrap(),
                    )
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
        std::fs::read(&target).expect("read installed target"),
        std::fs::read(&source).expect("read source")
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
fn login_shell_probe_uses_shell_resolved_path() {
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
    let shell = root.join("login-shell");
    std::fs::write(
        &shell,
        format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", asp.display()),
    )
    .expect("write probe shell");
    let mut shell_permissions = std::fs::metadata(&shell)
        .expect("inspect probe shell")
        .permissions();
    shell_permissions.set_mode(0o755);
    std::fs::set_permissions(&shell, shell_permissions).expect("chmod probe shell");

    let probe = super::probe_protocol_binary_in_login_shell(&shell);

    assert_eq!(
        probe,
        super::ProtocolBinaryShellProbe {
            shell,
            path: Some(asp),
            status: "found",
        }
    );
    std::fs::remove_dir_all(root).expect("cleanup probe root");
}

#[cfg(unix)]
#[test]
fn login_shell_probe_reports_missing_when_shell_cannot_resolve_asp() {
    use std::os::unix::fs::PermissionsExt;

    let root = protocol_binary_probe_root("missing");
    std::fs::create_dir_all(&root).expect("create probe root");
    let shell = root.join("login-shell");
    std::fs::write(&shell, b"#!/bin/sh\nexit 127\n").expect("write missing probe shell");
    let mut permissions = std::fs::metadata(&shell)
        .expect("inspect missing probe shell")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&shell, permissions).expect("chmod missing probe shell");

    let probe = super::probe_protocol_binary_in_login_shell(&shell);

    assert_eq!(
        probe,
        super::ProtocolBinaryShellProbe {
            shell,
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
