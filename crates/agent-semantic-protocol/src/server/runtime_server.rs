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

pub(crate) async fn await_agent_facing_runtime_server_client<F, T>(
    started: tokio::time::Instant,
    surface: &'static str,
    stage: &'static str,
    project_root: &Path,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    agent_facing_runtime_wait_remaining(started.elapsed(), surface, stage, project_root)?;
    let deadline = started + AGENT_FACING_EXECUTION_BUDGET;
    match tokio::time::timeout_at(deadline, future).await {
        Ok(result) => result,
        Err(_) => Err(agent_facing_wall_budget_error(
            surface,
            stage,
            started.elapsed(),
            project_root,
        )),
    }
}

const AGENT_FACING_EXECUTION_BUDGET: std::time::Duration = std::time::Duration::from_millis(800);
pub(crate) const RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(800);
pub(crate) const OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(5);
#[cfg(test)]
pub(crate) async fn linearize_reconcile_result_with_postcondition<F, Fut>(
    reconcile: Result<(), String>,
    postcondition: F,
) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    match reconcile {
        Ok(()) => Ok(()),
        Err(original) => match postcondition().await {
            Ok(()) => Ok(()),
            Err(_) => Err(original),
        },
    }
}

pub(crate) fn agent_facing_runtime_wait_remaining(
    elapsed: std::time::Duration,
    surface: &'static str,
    stage: &'static str,
    project_root: &Path,
) -> Result<std::time::Duration, String> {
    AGENT_FACING_EXECUTION_BUDGET
        .checked_sub(elapsed)
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| agent_facing_wall_budget_error(surface, stage, elapsed, project_root))
}

fn agent_facing_wall_budget_error(
    surface: &'static str,
    stage: &'static str,
    elapsed: std::time::Duration,
    project_root: &Path,
) -> String {
    let budget_micros =
        u64::try_from(AGENT_FACING_EXECUTION_BUDGET.as_micros()).unwrap_or(u64::MAX);
    if let Ok(state_home) = state_home() {
        let mut observation =
            agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                surface,
                stage,
                u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
                budget_micros,
                "budget-exceeded",
            );
        observation.failure_reason = Some("agent-facing-search-wall-budget-exceeded".to_owned());
        observation.retry_after_ms = Some(250);
        let admission_catalog_path = state_home
            .join("runtime")
            .join("server")
            .join("workspace-admissions.v1.json");
        if let Ok(admission) = agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::resolve_mapped(
            &admission_catalog_path,
            project_root,
        ) {
            observation.workspace_identity = Some(admission.workspace_identity);
        }
        observation.seal_budget_failure_identity();
        let _ =
            agent_semantic_client_db::runtime_server_opentelemetry::try_record_to_active_runtime(
                observation,
            );
    }
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.agent-facing-search-wall-failure",
        "schemaVersion": "1",
        "surface": surface,
        "state": "unavailable",
        "reasonKind": "agent-facing-search-wall-budget-exceeded",
        "stage": stage,
        "budgetMicros": budget_micros,
        "elapsedMicros": elapsed.as_micros(),
        "retryAfterMs": 250
    })
    .to_string()
}

pub(crate) async fn runtime_server_workspace_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_admitted_workspace_scope(project_root).await?;
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)).await?;
    Ok(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server_read_only(
            &endpoint,
            workspace_identity,
            canonical_project_root,
        ),
    )
}

pub(crate) async fn runtime_server_workspace_session_for_admission_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let state_home = state_home()?;
    agent_semantic_client_db::runtime_server_hook_admission_locator::connect_hook_workspace_session(
        &state_home,
        project_root,
    )
    .await
}

pub(super) async fn runtime_server_admitted_workspace_scope(
    project_root: &Path,
) -> Result<(String, PathBuf), String> {
    if !project_root.is_absolute() {
        return Err(format!(
            "Runtime Server query root must already be canonical and absolute: {}",
            project_root.display()
        ));
    }
    let catalog_path = state_home()?
        .join("runtime")
        .join("server")
        .join("workspace-admissions.v1.json");
    use agent_semantic_client_db::runtime_server_admission_catalog::{
        RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogResolveError,
    };
    let entry = RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, project_root)
        .map_err(|error: RuntimeWorkspaceAdmissionCatalogResolveError| error.to_string())?;
    Ok((entry.workspace_identity, entry.project_root))
}

pub(crate) use super::runtime_server_generation_data_plane::{
    RuntimeServerSearchDataPlane, RuntimeServerSearchSnapshot,
    runtime_server_search_data_plane_async,
};

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
    let endpoint_path = runtime_server_endpoint_path(&state_home);
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
    let endpoint_path = runtime_server_endpoint_path(&state_home);
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
        cleanup_endpoint(&state_home, &endpoint).await;
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

pub(crate) async fn await_healthy_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    await_healthy_runtime_server_with_budget(
        state_home,
        RUNTIME_SERVER_SUPERVISOR_EXECUTION_BUDGET,
        "detached lifecycle",
    )
    .await
}

pub(crate) async fn observe_agent_facing_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint = read_endpoint(&runtime_server_endpoint_path(state_home)).await?;
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
    )
    .await
}

async fn await_healthy_runtime_server_with_budget(
    state_home: &Path,
    budget: std::time::Duration,
    surface: &'static str,
) -> Result<RuntimeServerControlReceipt, String> {
    let started = tokio::time::Instant::now();
    let deadline = started + budget;
    loop {
        let observation = match healthcheck_runtime_server_at(state_home).await {
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy =>
            {
                return Ok(receipt);
            }
            Ok(receipt) => format!("state={:?} reason={:?}", receipt.state, receipt.reason),
            Err(error) => error,
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "{surface} did not publish a healthy Runtime Server within {}ms: {}",
                started.elapsed().as_millis(),
                observation
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

pub(crate) async fn healthcheck_runtime_server_at(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint = read_supervisor_endpoint(&runtime_server_endpoint_path(state_home)).await?;
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
    let canonical_runtime_artifact_digest =
        crate::command::protocol_binary::canonical_protocol_binary_artifact_digest(
            &canonical_runtime,
        )
        .await?;
    let artifact_action = runtime_server_artifact_action(
        &canonical_artifact,
        &running_artifact,
        &canonical_runtime_artifact_digest,
        &endpoint.runtime_artifact_digest,
    );
    let expected_transport_contract_digest =
        agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(
        );
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(state_home)
            .await?;
    let artifact_catalog_matches = endpoint.artifact_mode == artifact_catalog.mode_label()
        && endpoint.artifact_catalog_digest == artifact_catalog.digest();
    if !artifact_catalog_matches {
        return call_runtime_server(
            &endpoint,
            RuntimeServerOperation::Restart,
            canonical_runtime_artifact_digest,
            request_identity("healthcheck-artifact-catalog-restart").await?,
        )
        .await;
    }
    let receipt = match artifact_action {
        RuntimeServerArtifactAction::Status
            if endpoint.transport_contract_digest == expected_transport_contract_digest =>
        {
            call_runtime_server(
                &endpoint,
                RuntimeServerOperation::Status,
                canonical_runtime_artifact_digest.clone(),
                request_identity("healthcheck").await?,
            )
            .await?
        }
        RuntimeServerArtifactAction::Status | RuntimeServerArtifactAction::Restart => {
            agent_semantic_client_db::runtime_server_control::reconcile_runtime_server(
                &endpoint,
                canonical_runtime_artifact_digest.clone(),
                expected_transport_contract_digest,
                request_identity("healthcheck-reconcile").await?,
            )
            .await?
        }
    };
    let endpoint_present = tokio::fs::try_exists(&endpoint.socket_path)
        .await
        .map_err(|error| format!("failed to inspect Runtime Server endpoint: {error}"))?;
    if receipt.state
        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        && !endpoint_present
    {
        let mut unavailable = receipt;
        unavailable.state =
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting;
        unavailable.reason = Some("Runtime Server endpoint is not present".to_owned());
        return Ok(unavailable);
    }
    if receipt.state
        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        && tokio::net::UnixStream::connect(runtime_server_telemetry_query_socket_path(state_home))
            .await
            .is_err()
    {
        return call_runtime_server(
            &endpoint,
            RuntimeServerOperation::Restart,
            canonical_runtime_artifact_digest,
            request_identity("healthcheck-telemetry-restart").await?,
        )
        .await;
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

#[path = "graph_turbo_daemon.rs"]
mod graph_turbo_daemon;

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

pub(crate) fn runtime_server_telemetry_socket_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join("opentelemetry.sock")
}

fn runtime_server_telemetry_query_socket_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join("opentelemetry-query.sock")
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
use super::runtime_server_endpoint_io::{
    cleanup_endpoint, read_endpoint, read_supervisor_endpoint, remove_stale_socket,
};

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_lifecycle_cli.rs"]
mod lifecycle_cli_tests;
