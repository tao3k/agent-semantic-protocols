use agent_semantic_client_db::runtime_server_control::prewarm_runtime_server_status_memory;
use agent_semantic_client_db::{
    RuntimeServerControlReceipt, RuntimeServerOperation, call_runtime_server,
    runtime_server_endpoint_path,
};
use clap::{Command, CommandFactory, Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path = "runtime_server_agent_config.rs"]
mod runtime_server_agent_config;
#[path = "runtime_server_daemon.rs"]
mod runtime_server_daemon;
#[path = "runtime_server_stop.rs"]
mod runtime_server_stop;
#[path = "runtime_server_telemetry_command.rs"]
mod runtime_server_telemetry_command;

#[derive(Debug, Parser)]
#[command(name = "asp server", disable_help_subcommand = true)]
struct ServerArgs {
    #[command(subcommand)]
    command: ServerCommand,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    /// Start the single ASP Server owned by the ASP State Home.
    Start,
    Status,
    Restart,
    /// Drain and stop the single ASP Server owned by the ASP State Home.
    Stop,
    /// Query resident OpenTelemetry performance receipts without opening Turso.
    #[command(hide = true)]
    Telemetry(runtime_server_telemetry_command::TelemetryQueryArgs),
    /// Internal detached daemon entrypoint.
    #[command(hide = true)]
    Daemon,
}

pub(crate) fn runtime_server_command() -> Command {
    ServerArgs::command()
}

pub(crate) const RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(800);

pub(crate) async fn runtime_server_workspace_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_query_workspace_scope(project_root)?;
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)?).await?;
    Ok(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server_client(
            &endpoint,
            workspace_identity,
            canonical_project_root,
        ),
    )
}

pub(crate) async fn runtime_server_workspace_session_for_admission_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    // AgentSession/ChoicePlane registration is a global Runtime control-plane
    // operation. It requires a stable workspace identity, but it must not
    // depend on a source-generation locator, Hook mmap inbox, or admission
    // catalog. Binding through the global endpoint removes the registration ->
    // generation -> typed-agent recursion.
    agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
        project_root,
    )
    .await
}

pub(crate) async fn runtime_server_stateless_search_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
        project_root,
    )
    .await
}

pub(super) fn runtime_server_query_workspace_scope(
    project_root: &Path,
) -> Result<(String, PathBuf), String> {
    if !project_root.is_absolute() {
        return Err(format!(
            "Runtime Server query root must already be canonical and absolute: {}",
            project_root.display()
        ));
    }
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?;
    // Query derives only the stable workspace key. The Runtime data plane is
    // the sole authority for whether an immutable generation exists; catalog
    // admission, bootstrap, repair, and retry remain lifecycle-only actions.
    Ok((workspace_identity, project_root.to_path_buf()))
}

pub(crate) async fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    let parsed = ServerArgs::try_parse_from(
        std::iter::once("asp server".to_owned()).chain(args.iter().cloned()),
    )
    .map_err(|error| error.to_string())?;
    match parsed.command {
        ServerCommand::Daemon => runtime_server_daemon::run_daemon().await,
        ServerCommand::Start => run_start().await,
        ServerCommand::Status => run_status().await,
        ServerCommand::Restart => run_restart().await,
        ServerCommand::Stop => runtime_server_stop::run_stop().await,
        ServerCommand::Telemetry(args) => {
            runtime_server_telemetry_command::run_telemetry_query(args).await
        }
    }
}

pub(crate) async fn runtime_server_workspace_exact_projection_async(
    project_root: &Path,
    language_id: agent_semantic_client_core::LanguageId,
    projection_kind: &str,
    structural_selector: &str,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String>
{
    let projection_kind =
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
            projection_kind,
        )
        .map_err(|error| format!("decode exact projection kind: {error}"))?;
    super::runtime_server_generation_data_plane::runtime_server_workspace_exact_projection_async(
        project_root,
        language_id,
        projection_kind,
        structural_selector,
    )
    .await
}

async fn run_control_status() -> Result<(), String> {
    let state_home = state_home()?;
    let endpoint_path = runtime_server_endpoint_path(&state_home)?;
    let endpoint = read_endpoint(&endpoint_path).await?;
    let request_id = request_identity("control").await?;
    prewarm_runtime_server_status_memory(&endpoint).await?;
    let receipt = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        request_id,
    )
    .await?;
    print_receipt(&receipt).await
}

async fn await_healthy_runtime_server_after_spawn() -> Result<(), String> {
    const STARTUP_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);
    const PROBE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

    let executable = std::env::current_exe().map_err(|error| {
        format!("failed to resolve ASP executable for Runtime Server readiness: {error}")
    })?;
    let deadline = tokio::time::Instant::now() + STARTUP_BUDGET;
    let mut last_state = "endpoint-unpublished".to_owned();

    loop {
        let output = tokio::process::Command::new(&executable)
            .args(["server", "status"])
            .output()
            .await
            .map_err(|error| format!("failed to probe Runtime Server readiness: {error}"))?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().rev() {
                let Ok(receipt) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                if receipt.get("state").and_then(serde_json::Value::as_str) == Some("healthy") {
                    return Ok(());
                }
                if let Some(state) = receipt.get("state").and_then(serde_json::Value::as_str) {
                    last_state = state.to_owned();
                } else if let Some(reason) =
                    receipt.get("reason").and_then(serde_json::Value::as_str)
                {
                    last_state = reason.to_owned();
                }
                break;
            }
        } else {
            last_state = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "Runtime Server owner spawned without publishing a healthy endpoint within {}ms: state={last_state} reasonKind=runtime-server-endpoint-publication-timeout",
                STARTUP_BUDGET.as_millis()
            ));
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
}

async fn run_start() -> Result<(), String> {
    run_start_inner().await?;
    await_healthy_runtime_server_after_spawn().await
}

async fn run_start_inner() -> Result<(), String> {
    let state_home = state_home()?;
    match super::runtime_server_supervisor::ensure_runtime_server(&state_home, true).await? {
        Some(receipt) => print_receipt(&receipt).await,
        None => run_status().await,
    }
}

async fn run_status() -> Result<(), String> {
    let state_home = state_home()?;
    match run_control_status().await {
        Ok(()) => Ok(()),
        Err(reason) => {
            let spawn =
                super::runtime_server_supervisor::read_runtime_server_spawn_receipt(&state_home)
                    .await?;
            let receipt = serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-lifecycle-receipt.v1",
                "schemaVersion": "1",
                "state": "stopped",
                "processId": spawn.as_ref().map(|receipt| receipt.process_id),
                "nonce": spawn.as_ref().map(|receipt| receipt.nonce.as_str()),
                "reason": reason,
            });
            let mut stdout = tokio::io::stdout();
            stdout
                .write_all(format!("{receipt}\n").as_bytes())
                .await
                .map_err(|error| format!("write Runtime Server status receipt: {error}"))?;
            stdout
                .flush()
                .await
                .map_err(|error| format!("flush Runtime Server status receipt: {error}"))
        }
    }
}

async fn run_restart() -> Result<(), String> {
    run_restart_inner().await?;
    await_healthy_runtime_server_after_spawn().await
}

async fn run_restart_inner() -> Result<(), String> {
    let state_home = state_home()?;
    match restart_runtime_server_at(&state_home).await? {
        Some(receipt) => print_receipt(&receipt).await,
        None => run_status().await,
    }
}

async fn restart_runtime_server_at(
    state_home: &Path,
) -> Result<Option<super::runtime_server_supervisor::RuntimeServerSpawnReceipt>, String> {
    let endpoint_path = runtime_server_endpoint_path(&state_home)?;
    if let Ok(endpoint) = read_supervisor_endpoint(&endpoint_path).await {
        crate::server::runtime_server_exit_receipt::remove_stale(&state_home).await?;
        super::runtime_server_supervisor::request_runtime_server_drain(&state_home).await?;
        let exit = crate::server::runtime_server_exit_receipt::await_owner_exit(
            &state_home,
            endpoint.owner_epoch,
        )
        .await?;
        if !exit.clean_drain {
            return Err("Runtime Server restart stopped after a failed service drain".to_owned());
        }
        cleanup_endpoint(&state_home, &endpoint).await?;
    }
    super::runtime_server_supervisor::ensure_runtime_server(state_home, true).await
}

/// Reconcile a resident Runtime after its immutable provider catalog changed.
///
/// Provider installation never starts a Runtime that was not already running.
/// When an owner is resident, however, it must not retain the superseded
/// catalog digest: the Runtime control plane drains that owner and waits for a
/// replacement to publish readiness before the install receipt is returned.
pub(crate) async fn reconcile_runtime_server_after_provider_catalog_change(
    state_home: &Path,
    catalog_write: bool,
) -> Result<&'static str, String> {
    if !catalog_write {
        return Ok("current");
    }
    let endpoint_path = runtime_server_endpoint_path(state_home)?;
    if !tokio::fs::try_exists(&endpoint_path)
        .await
        .map_err(|error| format!("inspect Runtime Server endpoint after catalog write: {error}"))?
    {
        return Ok("not-running");
    }
    restart_runtime_server_at(state_home).await?;
    await_healthy_runtime_server_after_spawn().await?;
    Ok("restarted")
}

pub(crate) async fn reconcile_runtime_server_for_healthcheck(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    super::runtime_server_supervisor::reconcile_healthy_runtime_server(state_home).await
}

pub(crate) async fn observe_agent_facing_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint = read_endpoint(&runtime_server_endpoint_path(state_home)?).await?;
    let request_id = request_identity("session-choice-plane").await?;
    prewarm_runtime_server_status_memory(&endpoint).await?;
    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        request_id,
    )
    .await
}

pub(super) async fn observe_runtime_server_readiness(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint = read_supervisor_endpoint(&runtime_server_endpoint_path(state_home)?).await?;
    let mut receipt =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_cached_health_status(
            Path::new(&endpoint.status_memory_path),
            request_identity("startup-readiness").await?,
        )
        .await?;
    let endpoint_present = tokio::fs::try_exists(&endpoint.socket_path)
        .await
        .map_err(|error| format!("failed to inspect Runtime Server endpoint: {error}"))?;
    if receipt.state
        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        && !endpoint_present
    {
        receipt.state =
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting;
        receipt.reason = Some("Runtime Server endpoint is not present".to_owned());
    }
    Ok(receipt)
}

/// Read-only liveness probe for agent-session admission.
///
/// Unlike `healthcheck_runtime_server_at`, this function never reconciles,
/// restarts, or waits on the supervisor. Admission can therefore spend its
/// small read budget on one status request and hand repair to the separately
/// bounded supervisor path.
async fn print_receipt<T: serde::Serialize>(receipt: &T) -> Result<(), String> {
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

#[path = "runtime_server_singleton_socket.rs"]
mod singleton_socket;

pub(crate) fn state_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "ASP_STATE_HOME and HOME are both unset".to_owned())?;
    Ok(PathBuf::from(home).join(".agent-semantic-protocols"))
}

pub(crate) fn runtime_server_telemetry_socket_path(state_home: &Path) -> Result<PathBuf, String> {
    Ok(
        agent_semantic_client_db::runtime_server_runtime_base(state_home)?
            .join("opentelemetry.sock"),
    )
}

pub(crate) fn runtime_server_telemetry_query_socket_path(
    state_home: &Path,
) -> Result<PathBuf, String> {
    Ok(
        agent_semantic_client_db::runtime_server_runtime_base(state_home)?
            .join("opentelemetry-query.sock"),
    )
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
use super::runtime_server_endpoint_io::{
    cleanup_endpoint, read_endpoint, read_supervisor_endpoint, remove_stale_socket,
};

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_lifecycle_cli.rs"]
mod lifecycle_cli_tests;
