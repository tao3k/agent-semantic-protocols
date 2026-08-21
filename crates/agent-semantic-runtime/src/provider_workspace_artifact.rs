use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

const PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-workspace-install";
const PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedProviderWorkspaceArtifact {
    source_root: PathBuf,
    entrypoint: PathBuf,
}

impl VerifiedProviderWorkspaceArtifact {
    #[must_use]
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    #[must_use]
    pub fn entrypoint(&self) -> &Path {
        &self.entrypoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderWorkspaceArtifactError {
    InvalidContract(String),
    RepairRequired(String),
}

impl fmt::Display for ProviderWorkspaceArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract(message) => {
                write!(formatter, "invalid provider workspace contract: {message}")
            }
            Self::RepairRequired(message) => {
                write!(formatter, "provider workspace repair required: {message}")
            }
        }
    }
}

impl std::error::Error for ProviderWorkspaceArtifactError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderWorkspaceInstallDescriptor {
    schema_id: String,
    schema_version: String,
    language_id: String,
    provider_id: String,
    binary: String,
    workspace_artifact: WorkspaceArtifactDescriptor,
    workspace_build: WorkspaceBuildDescriptor,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceArtifactDescriptor {
    root: String,
    entrypoint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceBuildDescriptor {
    derived_paths: Vec<String>,
}

pub async fn resolve_verified_provider_workspace_artifact(
    configured_developer_root: &Path,
    source_root: &str,
    workspace_install: &str,
    expected_language_id: &str,
    expected_provider_id: &str,
    expected_binary: &str,
) -> Result<VerifiedProviderWorkspaceArtifact, ProviderWorkspaceArtifactError> {
    let developer_root = tokio::fs::canonicalize(configured_developer_root)
        .await
        .map_err(|error| {
            ProviderWorkspaceArtifactError::InvalidContract(format!(
                "canonicalize configured Developer Source {}: {error}",
                configured_developer_root.display()
            ))
        })?;
    let provider_source_root = canonical_relative_path(
        &developer_root,
        Path::new(source_root),
        false,
        "development.sourceRoot",
    )
    .await?;
    let descriptor_path = canonical_relative_path(
        &provider_source_root,
        Path::new(workspace_install),
        false,
        "development.workspaceInstall",
    )
    .await?;
    let descriptor_bytes = tokio::fs::read(&descriptor_path).await.map_err(|error| {
        ProviderWorkspaceArtifactError::InvalidContract(format!(
            "read workspace install descriptor {}: {error}",
            descriptor_path.display()
        ))
    })?;
    let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_slice(&descriptor_bytes)
        .map_err(|error| {
            ProviderWorkspaceArtifactError::InvalidContract(format!(
                "parse workspace install descriptor {}: {error}",
                descriptor_path.display()
            ))
        })?;

    validate_descriptor_identity(
        &descriptor,
        expected_language_id,
        expected_provider_id,
        expected_binary,
    )?;

    let artifact_path = joined_relative_path(
        &developer_root,
        Path::new(&descriptor.workspace_artifact.root),
        false,
        "workspaceArtifact.root",
    )?;
    let artifact_is_declared = descriptor.workspace_build.derived_paths.iter().any(|path| {
        joined_relative_path(
            &developer_root,
            Path::new(path),
            false,
            "workspaceBuild.derivedPaths",
        )
        .is_ok_and(|candidate| artifact_path == candidate || artifact_path.starts_with(candidate))
    });
    if !artifact_is_declared {
        return Err(ProviderWorkspaceArtifactError::InvalidContract(format!(
            "workspace artifact root is not declared in workspaceBuild.derivedPaths: {}",
            artifact_path.display()
        )));
    }

    let verified_source_root = tokio::fs::canonicalize(&artifact_path)
        .await
        .map_err(|error| {
            ProviderWorkspaceArtifactError::RepairRequired(format!(
                "workspace artifact is unavailable at {}: {error}",
                artifact_path.display()
            ))
        })?;
    ensure_within(&verified_source_root, &developer_root, "workspace artifact")?;

    let entrypoint = Path::new(&descriptor.workspace_artifact.entrypoint);
    validate_relative_path(entrypoint, true, "workspaceArtifact.entrypoint")?;
    let source_root_is_file = tokio::fs::metadata(&verified_source_root)
        .await
        .map_err(|error| {
            ProviderWorkspaceArtifactError::RepairRequired(format!(
                "read workspace artifact metadata {}: {error}",
                verified_source_root.display()
            ))
        })?
        .is_file();
    let verified_entrypoint = if source_root_is_file && entrypoint == Path::new(".") {
        verified_source_root.clone()
    } else {
        verified_source_root.join(entrypoint)
    };
    if !tokio::fs::metadata(&verified_entrypoint)
        .await
        .is_ok_and(|metadata| metadata.is_file())
    {
        return Err(ProviderWorkspaceArtifactError::RepairRequired(format!(
            "workspace artifact entrypoint is unavailable at {}",
            verified_entrypoint.display()
        )));
    }
    ensure_within(
        &tokio::fs::canonicalize(&verified_entrypoint)
            .await
            .map_err(|error| {
                ProviderWorkspaceArtifactError::RepairRequired(format!(
                    "canonicalize workspace artifact entrypoint {}: {error}",
                    verified_entrypoint.display()
                ))
            })?,
        &developer_root,
        "workspace artifact entrypoint",
    )?;

    Ok(VerifiedProviderWorkspaceArtifact {
        source_root: verified_source_root,
        entrypoint: verified_entrypoint,
    })
}

fn validate_descriptor_identity(
    descriptor: &ProviderWorkspaceInstallDescriptor,
    expected_language_id: &str,
    expected_provider_id: &str,
    expected_binary: &str,
) -> Result<(), ProviderWorkspaceArtifactError> {
    let actual = (
        descriptor.schema_id.as_str(),
        descriptor.schema_version.as_str(),
        descriptor.language_id.as_str(),
        descriptor.provider_id.as_str(),
        descriptor.binary.as_str(),
    );
    let expected = (
        PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID,
        PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION,
        expected_language_id,
        expected_provider_id,
        expected_binary,
    );
    if actual != expected {
        return Err(ProviderWorkspaceArtifactError::InvalidContract(format!(
            "identity mismatch: expected {expected:?}, found {actual:?}"
        )));
    }
    Ok(())
}

async fn canonical_relative_path(
    root: &Path,
    relative: &Path,
    allow_current_directory: bool,
    field: &str,
) -> Result<PathBuf, ProviderWorkspaceArtifactError> {
    let joined = joined_relative_path(root, relative, allow_current_directory, field)?;
    let canonical = tokio::fs::canonicalize(&joined).await.map_err(|error| {
        ProviderWorkspaceArtifactError::InvalidContract(format!(
            "canonicalize {field} {}: {error}",
            joined.display()
        ))
    })?;
    ensure_within(&canonical, root, field)?;
    Ok(canonical)
}

fn joined_relative_path(
    root: &Path,
    relative: &Path,
    allow_current_directory: bool,
    field: &str,
) -> Result<PathBuf, ProviderWorkspaceArtifactError> {
    validate_relative_path(relative, allow_current_directory, field)?;
    Ok(root.join(relative))
}

fn validate_relative_path(
    path: &Path,
    allow_current_directory: bool,
    field: &str,
) -> Result<(), ProviderWorkspaceArtifactError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ProviderWorkspaceArtifactError::InvalidContract(format!(
            "{field} must be a non-empty relative path: {}",
            path.display()
        )));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir if allow_current_directory => {}
            _ => {
                return Err(ProviderWorkspaceArtifactError::InvalidContract(format!(
                    "{field} contains a forbidden path component: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn ensure_within(
    path: &Path,
    root: &Path,
    label: &str,
) -> Result<(), ProviderWorkspaceArtifactError> {
    if !path.starts_with(root) {
        return Err(ProviderWorkspaceArtifactError::InvalidContract(format!(
            "{label} escapes configured Developer Source: path={} root={}",
            path.display(),
            root.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProviderWorkspaceArtifactError, resolve_verified_provider_workspace_artifact};

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
}
