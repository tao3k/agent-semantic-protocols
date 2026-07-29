//! Attachment and lifecycle admission for a workspace-scoped resident DB service.

use std::path::Path;

#[derive(Debug)]
pub(super) enum WorkspaceDbResidentSessionError {
    Unavailable {
        workspace_identity: String,
        endpoint_path: std::path::PathBuf,
        source: std::io::Error,
    },
    Invalid(String),
}

impl WorkspaceDbResidentSessionError {
    pub(super) fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }
}

impl std::fmt::Display for WorkspaceDbResidentSessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable {
                workspace_identity,
                endpoint_path,
                source,
            } => write!(
                formatter,
                "state=cold-required reasonKind=workspace-resident-service-required workspaceIdentity={workspace_identity} endpoint={} error={source}",
                endpoint_path.display()
            ),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WorkspaceDbResidentSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

impl From<String> for WorkspaceDbResidentSessionError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}

impl From<WorkspaceDbResidentSessionError> for String {
    fn from(error: WorkspaceDbResidentSessionError) -> Self {
        error.to_string()
    }
}

pub(super) fn session(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    WorkspaceDbResidentSessionError,
> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let endpoint_path = resolved
        .paths
        .hooks_dir
        .join("state")
        .join("workspace-db-resident-service-endpoint.v1.json");
    let bytes = std::fs::read(&endpoint_path).map_err(|source| {
        WorkspaceDbResidentSessionError::Unavailable {
            workspace_identity: resolved.workspace.workspace_id.as_str().to_owned(),
            endpoint_path: endpoint_path.clone(),
            source,
        }
    })?;
    let endpoint: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerEndpoint =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "invalid workspace resident DB service endpoint {}: {error}",
                endpoint_path.display()
            )
        })?;
    let owner_artifact = std::env::current_exe()
        .map_err(|error| format!("failed to resolve workspace DB owner artifact: {error}"))?;
    let owner_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(&owner_artifact)?.to_string();
    endpoint.validate_for_workspace(
        resolved.workspace.workspace_id.as_str(),
        &owner_artifact_digest,
    )?;
    Ok(agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::new(endpoint))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceResidentServiceEnsureReceipt {
    pub(super) status: &'static str,
    pub(super) workspace_identity: Option<String>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
}

fn canonical_git_workspace(cwd: &Path) -> Result<Option<std::path::PathBuf>, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| {
            format!(
                "failed to inspect Git workspace from {}: {error}",
                cwd.display()
            )
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    let root = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("Git workspace root is not UTF-8: {error}"))?
        .trim();
    if root.is_empty() {
        return Ok(None);
    }
    std::path::PathBuf::from(root)
        .canonicalize()
        .map(Some)
        .map_err(|error| format!("failed to canonicalize Git workspace {root}: {error}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceResidentServiceHealth {
    workspace_identity: String,
}

fn probe(workspace_root: &Path) -> Result<WorkspaceResidentServiceHealth, String> {
    let session = session(workspace_root).map_err(|error| error.to_string())?;
    super::workspace_db_runtime::block_on(session.health())
        .map_err(|error| error.to_string())??;
    Ok(WorkspaceResidentServiceHealth {
        workspace_identity: session.workspace_identity().to_owned(),
    })
}

pub(super) fn ensure(cwd: &Path) -> Result<WorkspaceResidentServiceEnsureReceipt, String> {
    let Some(workspace_root) = canonical_git_workspace(cwd)? else {
        return Ok(WorkspaceResidentServiceEnsureReceipt {
            status: "not-git-workspace",
            workspace_identity: None,
            workspace_root: None,
        });
    };
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(&workspace_root)?;
    if let Ok(health) = probe(&workspace_root) {
        return Ok(WorkspaceResidentServiceEnsureReceipt {
            status: "healthy",
            workspace_identity: Some(health.workspace_identity),
            workspace_root: Some(workspace_root),
        });
    }

    let executable = std::env::current_exe().map_err(|error| {
        format!("failed to resolve ASP executable for resident service: {error}")
    })?;
    let mut command = std::process::Command::new(&executable);
    command
        .args(["workspace-db", "resident", "serve", "--workspace"])
        .arg(&workspace_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map_err(|error| {
        format!(
            "failed to start workspace resident DB service for {} with {}: {error}",
            workspace_root.display(),
            executable.display()
        )
    })?;

    Ok(WorkspaceResidentServiceEnsureReceipt {
        status: "starting",
        workspace_identity: Some(resolved.workspace.workspace_id.as_str().to_owned()),
        workspace_root: Some(workspace_root),
    })
}
