use super::ProviderWorkspaceArtifactError;
use super::acquire_provider_workspace_build_guard;
use super::resolve_verified_provider_workspace_artifact;

fn descriptor(language_id: &str, artifact_root: &str) -> String {
    format!(
        r#"{{
                "schemaId":"agent.semantic-protocols.provider-workspace-install",
                "schemaVersion":"1",
                "schemaAuthority":"https://tao3k.github.io/agent-semantic-protocols/schemas/",
                "languageId":"{language_id}",
                "providerId":"asp-rust",
                "binary":"asp-rust",
                "workspaceArtifact":{{"root":"{artifact_root}","entrypoint":"."}},
                "workspaceBuild":{{"derivedPaths":["provider/target"]}}
            }}"#
    )
}

#[test]
fn workspace_build_guard_rejects_a_second_cross_process_writer() {
    let temp = tempfile::tempdir().expect("tempdir");
    let artifact = temp.path().join("build/workspace-provider");
    let first = acquire_provider_workspace_build_guard(&artifact).expect("first build owner");
    let error = acquire_provider_workspace_build_guard(&artifact)
        .err()
        .expect("second build owner must fail closed");
    assert!(error.contains("reasonKind=provider-workspace-build-active"));
    drop(first);
    acquire_provider_workspace_build_guard(&artifact).expect("released build owner");
}

#[tokio::test]
async fn resolves_existing_declared_developer_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider_root = temp.path().join("provider");
    let artifact = provider_root.join("target/asp-rust");
    std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
        .expect("create artifact parent");
    std::fs::write(&artifact, b"provider").expect("write artifact");
    std::fs::write(
        provider_root.join("install.json"),
        descriptor("rust", "provider/target/asp-rust"),
    )
    .expect("write descriptor");

    let resolved = resolve_verified_provider_workspace_artifact(
        temp.path(),
        "provider",
        "install.json",
        "rust",
        "asp-rust",
        "asp-rust",
    )
    .await
    .expect("resolve verified output");

    assert_eq!(resolved.entrypoint(), artifact.canonicalize().unwrap());
}

#[tokio::test]
async fn missing_developer_output_requires_repair() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider_root = temp.path().join("provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("install.json"),
        descriptor("rust", "provider/target/asp-rust"),
    )
    .expect("write descriptor");

    let error = resolve_verified_provider_workspace_artifact(
        temp.path(),
        "provider",
        "install.json",
        "rust",
        "asp-rust",
        "asp-rust",
    )
    .await
    .expect_err("missing output must not be accepted");

    assert!(matches!(
        error,
        ProviderWorkspaceArtifactError::RepairRequired(_)
    ));
}

#[tokio::test]
async fn descriptor_identity_drift_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let provider_root = temp.path().join("provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("install.json"),
        descriptor("julia", "provider/target/asp-rust"),
    )
    .expect("write descriptor");

    let error = resolve_verified_provider_workspace_artifact(
        temp.path(),
        "provider",
        "install.json",
        "rust",
        "asp-rust",
        "asp-rust",
    )
    .await
    .expect_err("identity drift must not be accepted");

    assert!(matches!(
        error,
        ProviderWorkspaceArtifactError::InvalidContract(_)
    ));
}
