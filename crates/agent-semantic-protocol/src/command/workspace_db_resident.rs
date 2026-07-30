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

fn load_session(
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
    endpoint.validate_for_workspace(resolved.workspace.workspace_id.as_str())?;
    Ok(agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::new(endpoint))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceResidentServiceEnsureReceipt {
    pub(super) status: &'static str,
    pub(super) workspace_identity: Option<String>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
    pub(super) transport_contract_digest: Option<String>,
    pub(super) owner_epoch: Option<u64>,
    pub(super) runtime_binary_path: Option<String>,
    pub(super) runtime_binary_digest: Option<String>,
}

fn canonical_workspace_root(cwd: &Path) -> Result<std::path::PathBuf, String> {
    agent_semantic_client_core::state_core::ResolvedState::resolve(cwd)
        .map(|resolved| resolved.workspace.root)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceResidentServiceHealth {
    workspace_identity: String,
    transport_contract_digest: String,
    owner_epoch: u64,
    runtime_binary_path: String,
    runtime_binary_digest: String,
}

async fn request_shutdown_and_wait(
    session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
) -> Result<(), String> {
    session.shutdown().await?;
    for _ in 0..100 {
        if session.health().await.is_err() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    Err("workspace resident owner did not close its transport within 100ms".to_owned())
}

async fn retire_unreachable_endpoint_owner(workspace_root: &Path) -> Result<bool, String> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(workspace_root)?;
    let runtime_base =
        agent_semantic_client_db::workspace_db_ipc::workspace_db_owner_runtime_base();
    let endpoint_path = resolved
        .paths
        .hooks_dir
        .join("state")
        .join("workspace-db-resident-service-endpoint.v1.json");
    let workspace_identity = resolved.workspace.workspace_id.as_str().to_owned();
    tokio::task::spawn_blocking(move || {
        agent_semantic_client_db::workspace_db_ipc::try_retire_workspace_db_owner_endpoint(
            &runtime_base,
            &workspace_identity,
            &endpoint_path,
        )
    })
    .await
    .map_err(|error| format!("workspace resident retirement task failed: {error}"))?
    .map(|disposition| {
        !matches!(
            disposition,
            agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerRetirement::ElectionHeld
        )
    })
}

pub(super) fn session(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    WorkspaceDbResidentSessionError,
> {
    if let Ok(session) = load_session(project_root) {
        if super::workspace_db_runtime::block_on(session.health())
            .is_ok_and(|health| health.is_ok())
        {
            return Ok(session);
        }
    }
    ensure(project_root)?;
    load_session(project_root)
}

async fn load_session_async(
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
    let bytes = tokio::fs::read(&endpoint_path).await.map_err(|source| {
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
    endpoint.validate_for_workspace(resolved.workspace.workspace_id.as_str())?;
    Ok(agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::new(endpoint))
}

async fn probe_async(workspace_root: &Path) -> Result<WorkspaceResidentServiceHealth, String> {
    let session = load_session_async(workspace_root)
        .await
        .map_err(|error| error.to_string())?;
    session.health().await?;
    Ok(WorkspaceResidentServiceHealth {
        workspace_identity: session.workspace_identity().to_owned(),
        transport_contract_digest: session.transport_contract_digest().to_owned(),
        owner_epoch: session.owner_epoch(),
        runtime_binary_path: session.runtime_binary_path().to_owned(),
        runtime_binary_digest: session.runtime_binary_digest().to_owned(),
    })
}

pub(super) fn inspect_endpoint_identity(
    workspace_root: &Path,
) -> Result<WorkspaceResidentServiceEnsureReceipt, String> {
    let workspace_root = canonical_workspace_root(workspace_root)?;
    let session = super::workspace_db_runtime::block_on(load_session_async(&workspace_root))?
        .map_err(|error| error.to_string())?;
    Ok(WorkspaceResidentServiceEnsureReceipt {
        status: "endpoint-current-unverified",
        workspace_identity: Some(session.workspace_identity().to_owned()),
        workspace_root: Some(workspace_root),
        transport_contract_digest: Some(session.transport_contract_digest().to_owned()),
        owner_epoch: Some(session.owner_epoch()),
        runtime_binary_path: Some(session.runtime_binary_path().to_owned()),
        runtime_binary_digest: Some(session.runtime_binary_digest().to_owned()),
    })
}

pub(super) fn ensure(cwd: &Path) -> Result<WorkspaceResidentServiceEnsureReceipt, String> {
    let state_home = std::env::var_os("ASP_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".agent-semantic-protocols"))
        })
        .ok_or_else(|| "canonical ASP State Home is unavailable".to_owned())?;
    let executable = state_home.join("runtime/bin/asp");
    super::workspace_db_runtime::block_on(async {
        let expected_runtime_digest =
            super::workspace_db_runtime::digest_runtime_binary(executable.clone())
                .await
                .map_err(|error| {
                    format!(
                        "failed to digest canonical ASP runtime {}: {error}",
                        executable.display()
                    )
                })?;
        ensure_with_runtime_identity_async(cwd, &executable, &expected_runtime_digest).await
    })?
}

pub(super) fn ensure_with_runtime_identity(
    cwd: &Path,
    executable: &Path,
    expected_runtime_digest: &str,
) -> Result<WorkspaceResidentServiceEnsureReceipt, String> {
    super::workspace_db_runtime::block_on(ensure_with_runtime_identity_async(
        cwd,
        executable,
        expected_runtime_digest,
    ))?
}

async fn ensure_with_runtime_identity_async(
    cwd: &Path,
    executable: &Path,
    expected_runtime_digest: &str,
) -> Result<WorkspaceResidentServiceEnsureReceipt, String> {
    let workspace_root = canonical_workspace_root(cwd)?;
    let mut probe = probe_async(&workspace_root).await;
    for _ in 0..2 {
        if probe.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        probe = probe_async(&workspace_root).await;
    }
    match probe {
        Ok(health) if health.runtime_binary_digest == expected_runtime_digest => {
            return Ok(WorkspaceResidentServiceEnsureReceipt {
                status: "healthy",
                workspace_identity: Some(health.workspace_identity),
                workspace_root: Some(workspace_root),
                transport_contract_digest: Some(health.transport_contract_digest),
                owner_epoch: Some(health.owner_epoch),
                runtime_binary_path: Some(health.runtime_binary_path),
                runtime_binary_digest: Some(health.runtime_binary_digest),
            });
        }
        Ok(_) => {
            let session = load_session_async(&workspace_root)
                .await
                .map_err(|error| error.to_string())?;
            request_shutdown_and_wait(&session).await?;
        }
        Err(_) => {
            if !retire_unreachable_endpoint_owner(&workspace_root).await? {
                return Err(format!(
                    "workspace resident owner election is held while its endpoint is not yet reachable: workspace={}",
                    workspace_root.display()
                ));
            }
        }
    }

    let mut command = std::process::Command::new(executable);
    command
        .args(["workspace-db", "resident", "serve", "--workspace"])
        .arg(&workspace_root)
        .arg("--runtime-binary-digest")
        .arg(expected_runtime_digest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut readiness = String::new();
    let readiness_timeout = std::time::Duration::from_millis(100);
    let bytes_read = {
        let mut child =
            super::workspace_db_runtime::spawn_resident_process(command).map_err(|error| {
                format!(
                    "failed to start workspace resident DB service for {} with {}: {error}",
                    workspace_root.display(),
                    executable.display()
                )
            })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            "workspace resident DB service readiness pipe is unavailable".to_owned()
        })?;
        let mut reader = tokio::io::BufReader::new(stdout);
        tokio::time::timeout(
            readiness_timeout,
            tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut readiness),
        )
        .await
        .map_err(|_| {
            format!(
                "workspace resident DB service readiness exceeded {}ms: workspace={}",
                readiness_timeout.as_millis(),
                workspace_root.display()
            )
        })?
        .map_err(|error| format!("failed to read workspace resident DB readiness receipt: {error}"))
    }?;
    if bytes_read == 0 || !readiness.starts_with("[workspace-resident-service-ready]") {
        if let Ok(health) = probe_async(&workspace_root).await
            && health.runtime_binary_digest == expected_runtime_digest
        {
            return Ok(WorkspaceResidentServiceEnsureReceipt {
                status: "healthy",
                workspace_identity: Some(health.workspace_identity),
                workspace_root: Some(workspace_root),
                transport_contract_digest: Some(health.transport_contract_digest),
                owner_epoch: Some(health.owner_epoch),
                runtime_binary_path: Some(health.runtime_binary_path),
                runtime_binary_digest: Some(health.runtime_binary_digest),
            });
        }
        return Err(format!(
            "workspace resident DB service exited before readiness: workspace={} receipt={}",
            workspace_root.display(),
            readiness.trim()
        ));
    }
    let health = probe_async(&workspace_root).await?;

    Ok(WorkspaceResidentServiceEnsureReceipt {
        status: "ready",
        workspace_identity: Some(health.workspace_identity),
        workspace_root: Some(workspace_root),
        transport_contract_digest: Some(health.transport_contract_digest),
        owner_epoch: Some(health.owner_epoch),
        runtime_binary_path: Some(health.runtime_binary_path),
        runtime_binary_digest: Some(health.runtime_binary_digest),
    })
}
