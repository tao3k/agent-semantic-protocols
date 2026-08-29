use super::{
    admit_embedded_hook_config, publish_embedded_hook_config, resolve_hook_binary_candidate,
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
fn hook_generation_binary_is_an_executable_sibling_of_the_installing_binary() {
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

#[cfg(unix)]
#[tokio::test]
async fn hook_binary_startup_validation_failure_preserves_previous_generation() {
    use agent_semantic_artifacts::hook_generation::{
        HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
        read_current_hook_generation,
    };
    use std::os::unix::fs::PermissionsExt;

    let state_home = tempfile::tempdir().expect("isolated Hook state");
    let write_hook_binary = |name: &str, body: &str| {
        let path = state_home.path().join(name);
        std::fs::write(&path, body).expect("write Hook binary fixture");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("Hook binary mode");
        path
    };
    let previous_binary =
        write_hook_binary("previous-asp", "#!/bin/sh\nprintf '%s\\n' 'asp previous'\n");
    let previous = prepare_hook_generation(
        state_home.path(),
        HookGenerationCandidate {
            hook_binary: &previous_binary,
            config: b"schemaVersion = 1",
            compiled_matcher: b"complete-matcher",
            registry: b"complete-registry",
        },
    )
    .expect("prepare previous generation");
    commit_hook_generation(state_home.path(), &previous).expect("commit previous generation");

    let failing_binary = write_hook_binary("failing-asp", "#!/bin/sh\nexit 7\n");
    let candidate = prepare_hook_generation(
        state_home.path(),
        HookGenerationCandidate {
            hook_binary: &failing_binary,
            config: b"schemaVersion = 1\nnext = true",
            compiled_matcher: b"next-complete-matcher",
            registry: b"next-complete-registry",
        },
    )
    .expect("prepare failing Hook binary generation");
    let error = agent_semantic_hook::candidate_validation::validate_hook_binary_candidate(
        agent_semantic_hook::candidate_validation::HookBinaryCandidateValidation {
            hook_binary_path: &candidate.receipt.hook_binary_path,
            generation_path: &candidate.receipt.generation_path,
            generation_digest: &candidate.receipt.generation_digest,
            state_home: state_home.path(),
        },
    )
    .await
    .expect_err("failing Hook binary must not become current");
    assert!(error.contains("failed validation"), "{error}");
    let current = read_current_hook_generation(state_home.path())
        .expect("read current generation")
        .expect("previous generation remains current");
    assert_eq!(
        current.generation_digest,
        previous.receipt.generation_digest
    );
}
