use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_control::prewarm_runtime_server_status_memory;
use agent_semantic_client_db::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerOperation,
    WorkspaceDbRegistry, acquire_runtime_server_election, call_runtime_server,
    prepare_runtime_server_endpoint, runtime_server_endpoint_path,
};
use clap::{Command, CommandFactory, Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Parser)]
#[command(name = "asp server", disable_help_subcommand = true)]
struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    Status,
    Reconcile,
    Restart,
    /// Run the long-lived ASP Runtime Server under the platform supervisor.
    Daemon,
}

pub(crate) fn runtime_server_command() -> Command {
    ServerArgs::command()
}

pub(super) fn block_on_runtime_server_client<F>(future: F) -> Result<F::Output, String>
where
    F: std::future::Future,
{
    agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()
        .map(|runtime| runtime.block_on(future))
}

pub(super) fn runtime_server_workspace_session(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    block_on_runtime_server_client(runtime_server_workspace_session_async(project_root))?
}

pub(super) async fn runtime_server_workspace_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let workspace_identity = runtime_server_workspace_identity(project_root)?;
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)).await?;
    Ok(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity,
        ),
    )
}

pub(super) fn runtime_server_workspace_identity(project_root: &Path) -> Result<String, String> {
    Ok(
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string(),
    )
}

pub(super) async fn runtime_server_workspace_generation_client_async(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient,
    String,
> {
    let workspace_identity = runtime_server_workspace_identity(project_root)?;
    let pointer_path = agent_semantic_client_db::runtime_server_runtime_base()
        .join("workspaces")
        .join(workspace_identity)
        .join("generations")
        .join("active-generation.pointer");
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneOpen;
    let recovery_reason =
        match agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient::open_state(
            &pointer_path,
        )
        .await?
        {
            WorkspaceGenerationDataPlaneOpen::Ready(client) => return Ok(client),
            WorkspaceGenerationDataPlaneOpen::Missing => "active-generation-missing".to_owned(),
            WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } => reason,
        };
    let ensure = runtime_server_workspace_session_async(project_root)
        .await?
        .ensure_runtime_generation(project_root)
        .await
        .map_err(|error| {
            format!(
                "Runtime Server generation recovery failed: workspace={} initialState={} error={error}",
                project_root.display(),
                recovery_reason
            )
        })?;
    if let agent_semantic_client_db::workspace_db_ipc::RuntimeGenerationEnsure::Admission(receipt) =
        ensure
    {
        return Err(format!(
            "cold-index-required workspaceIdentity={} state={:?} accepted={} attempt={}",
            receipt.workspace_identity, receipt.state, receipt.accepted, receipt.attempt
        ));
    }
    match agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient::open_state(
        &pointer_path,
    )
    .await?
    {
        WorkspaceGenerationDataPlaneOpen::Ready(client) => Ok(client),
        WorkspaceGenerationDataPlaneOpen::Missing => Err(format!(
            "Runtime Server restored no active generation pointer: workspace={} initialState={}",
            project_root.display(),
            recovery_reason
        )),
        WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } => Err(format!(
            "Runtime Server restored an invalid active generation: workspace={} initialState={} restoredState={reason}",
            project_root.display(),
            recovery_reason
        )),
    }
}

pub(super) fn runtime_server_current_source_index_snapshot(
    project_root: &Path,
) -> Result<agent_semantic_client::source_index::CurrentSourceIndexSnapshot, String> {
    block_on_runtime_server_client(async {
        let client = runtime_server_workspace_generation_client_async(project_root).await?;
        let lease = client.lease();
        lease.generation().validate()?;
        let workspace_generation =
            agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
                lease.generation().workspace_generation.clone(),
            )
            .map_err(|error| {
                format!("resident workspace generation evidence is incomplete: {error}")
            })?;
        let source_blobs =
            agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                lease.generation().owners.iter().map(|owner| {
                    (
                        agent_semantic_client_db::ClientDbSourceIndexPath::from(
                            owner.owner_path.as_str(),
                        ),
                        owner.bytes.clone(),
                    )
                }),
            );
        Ok(
            agent_semantic_client::source_index::CurrentSourceIndexSnapshot {
                workspace_snapshot: lease.generation().workspace_snapshot.clone(),
                source_snapshot: lease.generation().source_snapshot.clone(),
                workspace_generation,
                source_blobs,
            },
        )
    })?
}

pub(crate) fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    let parsed = ServerArgs::try_parse_from(
        std::iter::once("asp server".to_owned()).chain(args.iter().cloned()),
    )
    .map_err(|error| error.to_string())?;
    match parsed.command {
        ServerCommand::Daemon => agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_daemon()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to create ASP Runtime Server Tokio runtime: {error}"))?
            .block_on(run_daemon()),
        command => {
            block_on_runtime_server_client(async move {
                match command {
                    ServerCommand::Status => run_control(RuntimeServerOperation::Status).await,
                    ServerCommand::Reconcile => {
                        run_control(RuntimeServerOperation::Reconcile).await
                    }
                    ServerCommand::Restart => run_control(RuntimeServerOperation::Restart).await,
                    ServerCommand::Daemon => unreachable!("daemon handled before client runtime"),
                }
            })?
        }
    }
}

async fn run_control(operation: RuntimeServerOperation) -> Result<(), String> {
    let state_home = state_home()?;
    if operation == RuntimeServerOperation::Reconcile {
        super::runtime_server_supervisor::reconcile_runtime_server_supervisor(&state_home).await?;
        let runtime_artifact_digest =
            digest_file(&state_home.join("runtime").join("bin").join("asp")).await?;
        let receipt = RuntimeServerControlReceipt::starting(
            request_identity("control").await?,
            runtime_artifact_digest,
            "platform supervisor reconciliation completed".to_owned(),
        );
        return print_receipt(&receipt).await;
    }
    let endpoint_path = runtime_server_endpoint_path(&state_home);
    let endpoint = read_endpoint(&endpoint_path).await;
    let request_id = request_identity("control").await?;
    let endpoint = match endpoint {
        Ok(endpoint) => endpoint,
        Err(error) if operation == RuntimeServerOperation::Restart => {
            super::runtime_server_supervisor::reconcile_runtime_server_supervisor(&state_home)
                .await?;
            let runtime_artifact_path = state_home.join("runtime").join("bin").join("asp");
            let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
            let receipt = RuntimeServerControlReceipt::starting(
                request_id,
                runtime_artifact_digest,
                format!("platform supervisor reconcile requested: {error}"),
            );
            print_receipt(&receipt).await?;
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let expected_runtime_artifact_digest = if operation == RuntimeServerOperation::Status {
        prewarm_runtime_server_status_memory(&endpoint).await?;
        endpoint.runtime_artifact_digest.clone()
    } else {
        digest_file(&state_home.join("runtime").join("bin").join("asp")).await?
    };
    let receipt = match call_runtime_server(
        &endpoint,
        operation,
        expected_runtime_artifact_digest,
        request_id,
    )
    .await
    {
        Ok(receipt) => receipt,
        Err(error) if operation == RuntimeServerOperation::Restart => {
            super::runtime_server_supervisor::reconcile_runtime_server_supervisor(&state_home)
                .await?;
            RuntimeServerControlReceipt::starting(
                request_identity("control-recovery").await?,
                digest_file(&state_home.join("runtime").join("bin").join("asp")).await?,
                format!("platform supervisor recovered an unreachable Runtime Server: {error}"),
            )
        }
        Err(error) => return Err(error),
    };
    print_receipt(&receipt).await
}

pub(super) fn healthcheck_runtime_server() -> Result<RuntimeServerControlReceipt, String> {
    block_on_runtime_server_client(healthcheck_runtime_server_with_reconciliation())?
}

async fn healthcheck_runtime_server_with_reconciliation()
-> Result<RuntimeServerControlReceipt, String> {
    let protocol_home = state_home()?;
    match healthcheck_runtime_server_async().await {
        Ok(receipt) => Ok(receipt),
        Err(initial_error) => {
            super::runtime_server_supervisor::reconcile_runtime_server_supervisor(&protocol_home)
                .await
                .map_err(|reconcile_error| {
                    format!(
                        "Runtime Server health failed and supervisor reconciliation failed: \
                         health={initial_error}; reconciliation={reconcile_error}"
                    )
                })?;
            healthcheck_runtime_server_async()
                .await
                .map_err(|reconciled_error| {
                    format!(
                        "Runtime Server remained unhealthy after supervisor reconciliation: \
                         initial={initial_error}; reconciled={reconciled_error}"
                    )
                })
        }
    }
}

pub(crate) async fn healthcheck_runtime_server_async() -> Result<RuntimeServerControlReceipt, String>
{
    let state_home = state_home()?;
    healthcheck_runtime_server_at(&state_home).await
}

pub(crate) async fn healthcheck_runtime_server_at(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint = read_endpoint(&runtime_server_endpoint_path(state_home)).await?;
    let canonical_runtime = state_home.join("runtime").join("bin").join("asp");
    let canonical_artifact =
        tokio::fs::canonicalize(&canonical_runtime)
            .await
            .map_err(|error| {
                format!(
                    "failed to resolve canonical ASP runtime {}: {error}",
                    canonical_runtime.display()
                )
            })?;
    let running_artifact = tokio::fs::canonicalize(&endpoint.runtime_artifact_path)
        .await
        .map_err(|error| {
            format!(
                "failed to resolve running ASP Runtime Server artifact {}: {error}",
                endpoint.runtime_artifact_path
            )
        })?;
    let canonical_runtime_artifact_digest = digest_file(&canonical_runtime).await?;
    let operation = match runtime_server_artifact_action(
        &canonical_artifact,
        &running_artifact,
        &canonical_runtime_artifact_digest,
        &endpoint.runtime_artifact_digest,
    ) {
        RuntimeServerArtifactAction::Status => RuntimeServerOperation::Status,
        RuntimeServerArtifactAction::Restart => RuntimeServerOperation::Restart,
    };
    call_runtime_server(
        &endpoint,
        operation,
        canonical_runtime_artifact_digest,
        request_identity("healthcheck").await?,
    )
    .await
}

async fn print_receipt(receipt: &RuntimeServerControlReceipt) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("failed to encode Runtime Server receipt: {error}"))?;
    bytes.push(b'\n');
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(&bytes)
        .await
        .map_err(|error| format!("failed to write Runtime Server receipt: {error}"))?;
    stdout
        .flush()
        .await
        .map_err(|error| format!("failed to flush Runtime Server receipt: {error}"))?;
    Ok(())
}

async fn run_daemon() -> Result<(), String> {
    let election = acquire_runtime_server_election().await?;
    let state_home = state_home()?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest = digest_file(&runtime_artifact_path).await?;
    let provider_registry = std::sync::Arc::new(
        super::global_provider_catalog::global_provider_registry_snapshot_async().await?,
    );
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let endpoint = prepare_runtime_server_endpoint(
        &runtime_artifact_path,
        &runtime_artifact_digest,
        owner_epoch,
        &binding_token,
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    remove_stale_socket(&socket_path).await?;
    remove_stale_socket(&data_plane_socket_path).await?;

    let server = RuntimeServer::bind_and_publish(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        &runtime_server_endpoint_path(&state_home),
    )
    .await?
    .with_workspace_generation_builder(std::sync::Arc::new(
        move |_workspace_identity, project_root| {
            let provider_registry = std::sync::Arc::clone(&provider_registry);
            Box::pin(async move {
                agent_semantic_client::rebuild_source_index_with_registry_async(
                    project_root,
                    (*provider_registry).clone(),
                )
                .await
                .map(|_| ())
            })
        },
    ));
    let result = server.serve().await.map(|_| ());
    cleanup_endpoint(&state_home, &endpoint).await;
    drop(election);
    result
}

fn state_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "ASP_STATE_HOME and HOME are both unset".to_owned())?;
    Ok(PathBuf::from(home).join(".agent-semantic-protocols"))
}

async fn read_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate()?;
    Ok(endpoint)
}

async fn remove_stale_socket(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale Runtime Server socket {}: {error}",
            path.display()
        )),
    }
}

async fn cleanup_endpoint(state_home: &Path, endpoint: &RuntimeServerEndpoint) {
    let endpoint_path = runtime_server_endpoint_path(state_home);
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let status_memory_path = PathBuf::from(&endpoint.status_memory_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(data_plane_socket_path).await;
        let _ = tokio::fs::remove_file(status_memory_path).await;
    }
}

async fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path).await.map_err(|error| {
        format!(
            "failed to open ASP runtime artifact {}: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await.map_err(|error| {
            format!(
                "failed to read ASP runtime artifact {}: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

async fn request_identity(seed: &str) -> Result<String, String> {
    let entropy = os_entropy().await?;
    Ok(format!(
        "{:x}",
        Sha256::digest([entropy.as_slice(), seed.as_bytes()].concat())
    ))
}

async fn daemon_identity() -> Result<(u64, String), String> {
    let entropy = os_entropy().await?;
    let mut epoch_bytes = [0_u8; 8];
    epoch_bytes.copy_from_slice(&entropy[..8]);
    let owner_epoch = u64::from_le_bytes(epoch_bytes).max(1);
    Ok((owner_epoch, format!("{:x}", Sha256::digest(entropy))))
}

async fn os_entropy() -> Result<[u8; 32], String> {
    let mut source = tokio::fs::File::open("/dev/urandom")
        .await
        .map_err(|error| format!("failed to open OS entropy source: {error}"))?;
    let mut entropy = [0_u8; 32];
    source
        .read_exact(&mut entropy)
        .await
        .map_err(|error| format!("failed to read OS entropy: {error}"))?;
    Ok(entropy)
}
use super::runtime_server_artifact::{RuntimeServerArtifactAction, runtime_server_artifact_action};
