use super::{
    admit_embedded_hook_config, admit_embedded_hook_runtime_candidate,
    publish_embedded_hook_config, resolve_hook_binary_candidate,
    retire_legacy_hook_generation_pointer,
};

#[test]
fn canonical_binary_publication_materializes_its_matching_hook_contract() {
    let root = std::env::temp_dir().join(format!(
        "asp-binary-hook-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    admit_embedded_hook_config().expect("embedded Hook config is schema-valid");
    assert_eq!(
        publish_embedded_hook_config(&root).expect("publish embedded Hook config"),
        "created"
    );
    let config = std::fs::read_to_string(root.join("hooks/config.toml"))
        .expect("read published Hook config");
    assert!(config.contains(&format!(
        "contractFingerprint = \"{}\"",
        agent_semantic_config::hook_client_contract_fingerprint()
    )));
    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn canonical_runtime_hook_retires_only_the_legacy_generation_pointer() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("isolated State Home");
    let generations = root.path().join("hooks/generations/blake3-256/old");
    std::fs::create_dir_all(&generations).expect("legacy generation fixture");
    let current = root.path().join("hooks/current");
    symlink("generations/blake3-256/old", &current).expect("legacy current fixture");

    assert_eq!(
        retire_legacy_hook_generation_pointer(root.path()).expect("retire legacy selector"),
        "retired"
    );
    assert!(!current.exists());
    assert!(
        generations.exists(),
        "retirement must not recursively delete immutable history"
    );
    assert_eq!(
        retire_legacy_hook_generation_pointer(root.path()).expect("idempotent retirement"),
        "absent"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn hook_candidate_admission_is_content_only_and_never_spawns_the_candidate() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().expect("isolated build directory");
    let asp = root.path().join("asp");
    let hook = root.path().join("asp-hook");
    let execution_marker = root.path().join("candidate-was-executed");
    std::fs::write(&asp, b"asp fixture").expect("write ASP fixture");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\ntouch '{}'\n", execution_marker.display()),
    )
    .expect("write executable Hook fixture");
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).expect("Hook fixture mode");

    let admitted = admit_embedded_hook_runtime_candidate(&asp)
        .await
        .expect("content-addressed Hook admission");
    assert_eq!(admitted.source, hook);
    assert!(admitted.artifact_digest.starts_with("blake3-256:"));
    assert!(
        !execution_marker.exists(),
        "publication admission must not execute candidate artifacts"
    );
}

#[test]
fn legacy_generation_directory_conflict_fails_closed_without_deletion() {
    let root = tempfile::tempdir().expect("isolated State Home");
    let current = root.path().join("hooks/current");
    std::fs::create_dir_all(&current).expect("conflicting directory fixture");

    let error = retire_legacy_hook_generation_pointer(root.path())
        .expect_err("directory conflict must fail closed");
    assert!(error.contains("legacy-hook-generation-path-conflict"));
    assert!(current.is_dir());
}

#[cfg(unix)]
#[test]
fn hook_runtime_binary_is_an_executable_sibling_of_the_installing_binary() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().expect("isolated build directory");
    let asp = root.path().join("asp");
    let hook_binary = root.path().join("asp-hook");
    std::fs::write(&asp, b"asp fixture").expect("write ASP fixture");
    std::fs::write(&hook_binary, b"Hook fixture").expect("write Hook fixture");
    let mut permissions = std::fs::metadata(&hook_binary).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook_binary, permissions).expect("Hook binary mode");

    assert_eq!(
        resolve_hook_binary_candidate(&asp).expect("resolve Hook sibling"),
        hook_binary
    );
}

#[cfg(unix)]
#[test]
fn hook_binary_candidate_rejects_missing_symlink_and_non_executable_inputs() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = tempfile::tempdir().expect("isolated build directory");
    let asp = root.path().join("asp");
    let hook_binary = root.path().join("asp-hook");
    std::fs::write(&asp, b"asp fixture").expect("write ASP fixture");

    let missing = resolve_hook_binary_candidate(&asp).expect_err("missing Hook must fail closed");
    assert!(missing.contains("candidate is unavailable"), "{missing}");

    std::fs::write(&hook_binary, b"non-executable Hook").expect("write Hook fixture");
    let non_executable =
        resolve_hook_binary_candidate(&asp).expect_err("non-executable Hook must fail closed");
    assert!(
        non_executable.contains("not an executable regular file"),
        "{non_executable}"
    );

    std::fs::remove_file(&hook_binary).expect("remove Hook fixture");
    let target = root.path().join("hook-target");
    std::fs::write(&target, b"Hook target").expect("write Hook target");
    let mut permissions = std::fs::metadata(&target).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&target, permissions).expect("target mode");
    symlink(&target, &hook_binary).expect("create Hook symlink");
    let symlink_error =
        resolve_hook_binary_candidate(&asp).expect_err("symlink Hook must fail closed");
    assert!(
        symlink_error.contains("not an executable regular file"),
        "{symlink_error}"
    );
}
