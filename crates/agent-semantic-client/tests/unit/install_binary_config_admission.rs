use super::{
    admit_embedded_hook_config, publish_embedded_hook_config, validate_evaluator_candidate,
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
#[tokio::test]
async fn evaluator_startup_validation_failure_preserves_previous_generation() {
    use agent_semantic_artifacts::hook_generation::{
        HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
        read_current_hook_generation,
    };
    use std::os::unix::fs::PermissionsExt;

    let state_home = tempfile::tempdir().expect("isolated Hook state");
    let write_evaluator = |name: &str, body: &str| {
        let path = state_home.path().join(name);
        std::fs::write(&path, body).expect("write evaluator fixture");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("evaluator mode");
        path
    };
    let previous_binary =
        write_evaluator("previous-asp", "#!/bin/sh\nprintf '%s\\n' 'asp previous'\n");
    let previous = prepare_hook_generation(
        state_home.path(),
        HookGenerationCandidate {
            evaluator_binary: &previous_binary,
            config: b"schemaVersion = 1",
            compiled_matcher: b"complete-matcher",
            registry: b"complete-registry",
        },
    )
    .expect("prepare previous generation");
    commit_hook_generation(state_home.path(), &previous).expect("commit previous generation");

    let failing_binary = write_evaluator("failing-asp", "#!/bin/sh\nexit 7\n");
    let candidate = prepare_hook_generation(
        state_home.path(),
        HookGenerationCandidate {
            evaluator_binary: &failing_binary,
            config: b"schemaVersion = 1\nnext = true",
            compiled_matcher: b"next-complete-matcher",
            registry: b"next-complete-registry",
        },
    )
    .expect("prepare failing evaluator generation");
    let error = validate_evaluator_candidate(&candidate.receipt, state_home.path())
        .await
        .expect_err("failing evaluator must not become current");
    assert!(error.contains("startup validation"), "{error}");
    let current = read_current_hook_generation(state_home.path())
        .expect("read current generation")
        .expect("previous generation remains current");
    assert_eq!(
        current.generation_digest,
        previous.receipt.generation_digest
    );
}
