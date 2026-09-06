use agent_semantic_artifacts::installed_provider_binding::InstalledProviderArtifactIdentity;
use agent_semantic_artifacts::installed_provider_binding::InstalledProviderBinding;
use agent_semantic_artifacts::installed_provider_binding::InstalledProviderBindingInput;
use agent_semantic_artifacts::installed_provider_binding::RuntimeProviderExecutionBinding;
use agent_semantic_artifacts::installed_provider_binding::load_installed_provider_binding;
use agent_semantic_artifacts::installed_provider_binding::publish_installed_provider_binding;

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn input() -> InstalledProviderBindingInput {
    InstalledProviderBindingInput {
        provider_registration_digest: digest('b'),
        hook_policy_digest: digest('d'),
        providers: vec![InstalledProviderArtifactIdentity {
            language_id: "rust".to_owned(),
            provider_id: "asp-rust".to_owned(),
            artifact_digest: digest('f'),
            entrypoint_digest: digest('0'),
            artifact_metadata_digest: digest('1'),
            execution_command_digest: digest('2'),
        }],
    }
}

#[test]
fn provider_binding_identity_is_independent_from_runtime_binary_identity() {
    let binding = InstalledProviderBinding::build(input()).expect("binding");
    assert!(!binding.requires_refresh(&input()));
    assert_eq!(
        binding.generation,
        InstalledProviderBinding::build(input())
            .expect("same provider closure")
            .generation
    );
}

#[test]
fn binding_rejects_generation_drift() {
    let mut binding = InstalledProviderBinding::build(input()).expect("binding");
    binding.providers[0].artifact_digest = digest('8');
    assert_eq!(
        binding.validate().expect_err("generation drift"),
        "installed provider binding generation drift"
    );
}

#[test]
fn execution_binding_refreshes_for_each_workspace_authority() {
    let binding = RuntimeProviderExecutionBinding::build(
        "repo-project".to_owned(),
        "workspace-checkout".to_owned(),
        digest('a'),
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
    .expect("execution binding");
    for changed in 0..7 {
        let mut inputs = [
            "repo-project".to_owned(),
            "workspace-checkout".to_owned(),
            digest('a'),
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        ];
        inputs[changed] = match changed {
            0 => "repo-other".to_owned(),
            1 => "workspace-other".to_owned(),
            _ => digest('9'),
        };
        let refreshed = RuntimeProviderExecutionBinding::build(
            inputs[0].clone(),
            inputs[1].clone(),
            inputs[2].clone(),
            inputs[3].clone(),
            inputs[4].clone(),
            inputs[5].clone(),
            inputs[6].clone(),
        )
        .expect("refreshed execution binding");
        assert_ne!(binding.generation, refreshed.generation);
    }
}

#[test]
fn execution_binding_rejects_noncanonical_project_and_workspace_ids() {
    let error = RuntimeProviderExecutionBinding::build(
        "project-from-path".to_owned(),
        "workspace-checkout".to_owned(),
        digest('a'),
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
    .expect_err("projectId must be a RepoId");
    assert!(error.contains("canonical RepoId"), "{error}");

    let error = RuntimeProviderExecutionBinding::build(
        "repo-project".to_owned(),
        "/tmp/checkout".to_owned(),
        digest('a'),
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
    .expect_err("workspaceId must not be a path");
    assert!(error.contains("canonical WorkspaceId"), "{error}");
}

#[test]
fn publication_refreshes_and_enforces_cas() {
    let root = tempfile::tempdir().expect("state home");
    let first =
        publish_installed_provider_binding(root.path(), input(), None).expect("first publication");
    assert!(first.artifact_write);
    let warm =
        publish_installed_provider_binding(root.path(), input(), Some(first.generation.as_str()))
            .expect("warm publication");
    assert!(!warm.artifact_write);
    let mut changed = input();
    changed.hook_policy_digest = digest('9');
    let conflict = publish_installed_provider_binding(
        root.path(),
        changed.clone(),
        Some("blake3-256:deadbeef"),
    )
    .expect_err("stale publisher must lose");
    assert!(conflict.contains("publication conflict"), "{conflict}");
    let refreshed =
        publish_installed_provider_binding(root.path(), changed, Some(first.generation.as_str()))
            .expect("refresh publication");
    assert!(refreshed.artifact_write);
    assert_ne!(refreshed.generation, first.generation);
    assert_eq!(
        load_installed_provider_binding(root.path())
            .expect("load binding")
            .expect("published binding")
            .generation,
        refreshed.generation
    );
}
