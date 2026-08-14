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
pub(crate) const OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(5);
const OPERATOR_RUNTIME_SERVER_ACTIVE_STARTUP_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(30);

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

async fn run_start() -> Result<(), String> {
    let state_home = state_home()?;
    prepare_runtime_server_start(&state_home).await?;
    super::runtime_server_supervisor::ensure_runtime_server(&state_home, true).await?;
    let receipt = await_operator_runtime_server(&state_home).await?;
    print_receipt(&receipt).await
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
    let state_home = state_home()?;
    prepare_runtime_server_start(&state_home).await?;
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
    super::runtime_server_supervisor::ensure_runtime_server(&state_home, true).await?;
    let receipt = await_operator_runtime_server(&state_home).await?;
    print_receipt(&receipt).await
}

async fn prepare_runtime_server_start(state_home: &Path) -> Result<(), String> {
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(state_home)
            .await?;
    runtime_server_agent_config::synchronize_for_reconcile(&artifact_catalog, state_home).await?;
    crate::command::reconcile_global_provider_catalog_for_runtime(state_home)?;
    Ok(())
}

pub(crate) async fn reconcile_runtime_server_for_healthcheck(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    prepare_runtime_server_start(state_home).await?;
    super::runtime_server_supervisor::reconcile_healthy_runtime_server(state_home).await
}

pub(crate) async fn await_healthy_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    await_healthy_runtime_server_with_budget(
        state_home,
        RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
        "detached lifecycle",
        None,
    )
    .await
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

async fn await_operator_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    await_healthy_runtime_server_with_budget(
        state_home,
        OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET,
        "operator lifecycle",
        Some(OPERATOR_RUNTIME_SERVER_ACTIVE_STARTUP_BUDGET),
    )
    .await
}

async fn await_healthy_runtime_server_with_budget(
    state_home: &Path,
    budget: std::time::Duration,
    surface: &'static str,
    active_startup_budget: Option<std::time::Duration>,
) -> Result<RuntimeServerControlReceipt, String> {
    let started = tokio::time::Instant::now();
    let mut deadline = started + budget;
    let mut startup_progress_observed = false;
    loop {
        let observation = match observe_runtime_server_readiness(state_home).await {
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy =>
            {
                return Ok(receipt);
            }
            Ok(receipt) => format!("state={:?} reason={:?}", receipt.state, receipt.reason),
            Err(error) => error,
        };
        let owner_exit =
            crate::server::runtime_server_exit_receipt::read_latest_owner_exit(state_home)
                .await
                .map_err(|error| format!("read Runtime Server exit receipt: {error}"))?;
        if let Some(exit) = owner_exit {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-supervisor-owner-exited",
                "schemaVersion": "1",
                "surface": surface,
                "state": "unavailable",
                "reasonKind": "runtime-server-owner-exited",
                "observation": observation,
                "ownerEpoch": exit.owner_epoch,
                "cleanDrain": exit.clean_drain,
                "errors": exit.errors,
            })
            .to_string());
        }
        if !startup_progress_observed
            && let Some(active_budget) = active_startup_budget
            && tokio::fs::metadata(state_home.join("runtime/server/daemon-startup.v1.json"))
                .await
                .is_ok()
        {
            startup_progress_observed = true;
            deadline = started + active_budget;
        }
        if tokio::time::Instant::now() >= deadline {
            let short_owner_stderr_path =
                agent_semantic_client_db::runtime_server_runtime_base(state_home)?
                    .join("owner-stderr.log");
            let legacy_owner_stderr_path = state_home
                .join("runtime")
                .join("server")
                .join("owner-stderr.log");
            let owner_stderr = match tokio::fs::read_to_string(&short_owner_stderr_path).await {
                Ok(stderr) => stderr,
                Err(_) => tokio::fs::read_to_string(&legacy_owner_stderr_path)
                    .await
                    .unwrap_or_default(),
            };
            let owner_stderr = owner_stderr
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ");
            let spawn_receipt =
                crate::server::runtime_server_supervisor::read_runtime_server_spawn_receipt(
                    state_home,
                )
                .await?
                .and_then(|receipt| serde_json::to_string(&receipt).ok())
                .unwrap_or_else(|| "unavailable".to_owned());
            let startup_receipt =
                tokio::fs::read_to_string(state_home.join("runtime/server/daemon-startup.v1.json"))
                    .await
                    .unwrap_or_else(|_| "unavailable".to_owned());
            return Err(format!(
                "{surface} did not publish a healthy Runtime Server within {}ms: {}; spawnReceipt={}; startupReceipt={}; ownerStderr={}",
                started.elapsed().as_millis(),
                observation,
                spawn_receipt,
                startup_receipt,
                if owner_stderr.is_empty() {
                    "unavailable"
                } else {
                    owner_stderr.as_str()
                }
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

async fn observe_runtime_server_readiness(
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

fn runtime_server_telemetry_query_socket_path(state_home: &Path) -> Result<PathBuf, String> {
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
