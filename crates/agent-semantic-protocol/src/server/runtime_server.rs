use agent_semantic_client_db::runtime_server_control::prewarm_runtime_server_status_memory;
use agent_semantic_client_db::{
    RuntimeServerControlReceipt, RuntimeServerOperation, call_runtime_server_for_state_home,
};
use clap::{Command, CommandFactory, Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path = "runtime_server_daemon.rs"]
mod runtime_server_daemon;
#[path = "runtime_server_identity_handoff.rs"]
mod runtime_server_identity_handoff;
#[path = "runtime_server_search_service.rs"]
mod runtime_server_search_service;
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

pub(crate) async fn runtime_server_workspace_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_query_workspace_scope(project_root)?;
    let state_home = state_home()?;
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            &state_home,
        )
        .await?;
    let endpoint = read_endpoint(&endpoint_path).await?;
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
        ServerCommand::Stop => runtime_server_stop::run_stop().await,
        ServerCommand::Telemetry(args) => {
            runtime_server_telemetry_command::run_telemetry_query(args).await
        }
    }
}

async fn run_control_status() -> Result<(), String> {
    let state_home = state_home()?;
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            &state_home,
        )
        .await?;
    let endpoint = read_endpoint(&endpoint_path).await?;
    validate_runtime_server_service_publication(&endpoint).await?;
    let request_id = request_identity("control").await?;
    prewarm_runtime_server_status_memory(&endpoint).await?;
    let receipt = call_runtime_server_for_state_home(
        &state_home,
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        request_id,
    )
    .await?;
    print_receipt(&receipt).await
}

async fn run_start() -> Result<(), String> {
    run_start_inner().await?;
    run_status().await
}

async fn run_start_inner() -> Result<(), String> {
    let state_home = state_home()?;
    let event = operator_start_activation_event(&state_home)
        .await?
        .ok_or_else(|| {
        "state=runtime-server-activation-unavailable reasonKind=pending-activation-event-missing"
            .to_owned()
    })?;
    super::runtime_server_wire_adapter::ensure_healthy_runtime_server_for_activation_event(
        &state_home,
        &event,
        None,
    )
    .await?;
    Ok(())
}

async fn operator_start_activation_event(
    state_home: &Path,
) -> Result<
    Option<agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactActivationEvent>,
    String,
> {
    if let Some(event) =
        agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(
            state_home,
        )
        .await?
    {
        return Ok(Some(event));
    }
    agent_semantic_artifacts::runtime_artifact_publication::read_applied_runtime_artifact_activation_event(
        state_home,
    )
    .await
}

async fn run_status() -> Result<(), String> {
    let state_home = state_home()?;
    match run_control_status().await {
        Ok(()) => Ok(()),
        Err(reason) => {
            let spawn =
                super::runtime_server_wire_adapter::read_runtime_server_spawn_receipt(&state_home)
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

pub(crate) async fn ensure_runtime_server_for_healthcheck(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    observe_runtime_server_readiness(state_home).await
}

pub(super) async fn observe_runtime_server_readiness(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            state_home,
        )
        .await?;
    let endpoint = read_supervisor_endpoint(&endpoint_path).await?;
    let mut receipt =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_cached_health_status(
            Path::new(&endpoint.status_memory_path),
            request_identity("startup-readiness").await?,
        )
        .await?;
    if receipt.state
        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        && let Err(error) = validate_runtime_server_service_publication(&endpoint).await
    {
        receipt.state =
            agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting;
        receipt.reason = Some(error);
    }
    Ok(receipt)
}

async fn validate_runtime_server_service_publication(
    endpoint: &agent_semantic_client_db::RuntimeServerEndpoint,
) -> Result<(), String> {
    let control = tokio::fs::try_exists(&endpoint.socket_path);
    let data = tokio::fs::try_exists(&endpoint.data_plane_socket_path);
    let provider = tokio::fs::try_exists(&endpoint.provider_plane_socket_path);
    let (control, data, provider) = tokio::try_join!(control, data, provider)
        .map_err(|error| format!("failed to inspect Runtime Server service generation: {error}"))?;
    if control && data && provider {
        return Ok(());
    }
    Err(format!(
        "Runtime Server service generation is incomplete: ownerEpoch={} control={control} data={data} provider={provider}",
        endpoint.owner_epoch
    ))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeServerClientBootstrapDisposition {
    Continue,
    Terminal,
}

pub(crate) fn runtime_server_client_bootstrap_continues(
    disposition: RuntimeServerClientBootstrapDisposition,
) -> bool {
    disposition == RuntimeServerClientBootstrapDisposition::Continue
}

pub(crate) fn runtime_server_client_bootstrap_receipt(
    outcome: agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome,
    activation_generation: u64,
    artifact_digest: String,
    authority: &agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapAuthority,
) -> Option<agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapReceipt> {
    use agent_semantic_client_db::runtime_server_control::{
        RuntimeServerClientBootstrapReceipt, RuntimeServerState,
    };

    match outcome {
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident => {
            None
        }
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted => {
            Some(RuntimeServerClientBootstrapReceipt::new(
                RuntimeServerState::Starting,
                "runtime-server-activation-spawn-accepted",
                Some(activation_generation),
                Some(artifact_digest),
                "observe-runtime-server-activation",
                authority.clone(),
            ))
        }
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::OwnerStale => {
            Some(RuntimeServerClientBootstrapReceipt::new(
                RuntimeServerState::Degraded,
                "runtime-server-owner-stale",
                Some(activation_generation),
                Some(artifact_digest),
                "inspect-runtime-server-owner-receipt",
                authority.clone(),
            ))
        }
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::Failed => {
            Some(RuntimeServerClientBootstrapReceipt::new(
                RuntimeServerState::Degraded,
                "runtime-server-supervisor-failed",
                Some(activation_generation),
                Some(artifact_digest),
                "inspect-runtime-server-supervisor-receipt",
                authority.clone(),
            ))
        }
    }
}

pub(crate) async fn reconcile_pending_runtime_activation_for_client_bootstrap()
-> Result<RuntimeServerClientBootstrapDisposition, String> {
    let state_resolution = agent_semantic_runtime::state_core::resolve_state_home_projection()?;
    let state_home = state_resolution.state_home.clone();
    let authority = agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapAuthority {
        cwd: std::env::current_dir()
            .map_err(|error| format!("resolve Runtime client bootstrap cwd: {error}"))?,
        executable_path: std::env::current_exe()
            .map_err(|error| format!("resolve Runtime client bootstrap executable: {error}"))?,
        state_home: state_home.clone(),
        state_home_source: state_resolution.source,
        asp_state_home_present: state_resolution.asp_state_home_present,
        home_present: state_resolution.home_present,
        pending_activation_path: agent_semantic_artifacts::runtime_artifact_publication::runtime_artifact_activation_event_path(&state_home),
        applied_activation_path: state_home.join("runtime/activation/applied.json"),
        runtime_endpoint_path: agent_semantic_client_db::runtime_server_endpoint_path(&state_home)?,
    };
    let Some(activation_event) = operator_start_activation_event(&state_home).await? else {
        // Reusing an already-published endpoint is the only no-activation path
        // that depends on owner authority.  Validate that authority before
        // admitting the endpoint: a legacy owner observation is typed stale,
        // while malformed or unknown observations remain fail-closed.
        let current_owner =
            agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
                .await?;
        if matches!(
            observe_runtime_server_readiness(&state_home).await,
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
                    && current_owner.is_some()
        ) {
            return Ok(RuntimeServerClientBootstrapDisposition::Continue);
        }
        print_receipt(
            &agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapReceipt::new(
                agent_semantic_client_db::runtime_server_control::RuntimeServerState::Degraded,
                "runtime-server-activation-unavailable",
                None,
                None,
                "publish-runtime-artifact-activation",
                authority.clone(),
            ),
        )
        .await?;
        return Ok(RuntimeServerClientBootstrapDisposition::Terminal);
    };

    if !agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
        &state_home,
        activation_event.activation_generation,
    )
    .await?
    {
        print_receipt(
            &agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapReceipt::new(
                agent_semantic_client_db::runtime_server_control::RuntimeServerState::Degraded,
                "runtime-server-operator-stopped",
            Some(activation_event.activation_generation),
            Some(activation_event.artifact_digest.to_string()),
                "publish-newer-runtime-artifact-activation",
                authority.clone(),
            ),
        )
        .await?;
        return Ok(RuntimeServerClientBootstrapDisposition::Terminal);
    }

    let outcome = super::runtime_server_wire_adapter::ensure_runtime_server_for_activation_event(
        &state_home,
        &activation_event,
        activation_event.previous_artifact_digest.as_ref(),
    )
    .await?;
    if outcome
        == agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident
        && !matches!(
            observe_runtime_server_readiness(&state_home).await,
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        )
    {
        print_receipt(
            &agent_semantic_client_db::runtime_server_control::RuntimeServerClientBootstrapReceipt::new(
                agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting,
                "runtime-server-activation-owner-starting",
                Some(activation_event.activation_generation),
                Some(activation_event.artifact_digest.to_string()),
                "observe-runtime-server-activation",
                authority.clone(),
            ),
        )
        .await?;
        return Ok(RuntimeServerClientBootstrapDisposition::Terminal);
    }
    let Some(receipt) = runtime_server_client_bootstrap_receipt(
        outcome,
        activation_event.activation_generation,
        activation_event.artifact_digest.to_string(),
        &authority,
    ) else {
        return Ok(RuntimeServerClientBootstrapDisposition::Continue);
    };
    print_receipt(&receipt).await?;
    Ok(RuntimeServerClientBootstrapDisposition::Terminal)
}

pub(crate) fn state_home() -> Result<PathBuf, String> {
    agent_semantic_runtime::state_core::resolve_state_home()
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
    // The supervisor publishes the child PID before the daemon can publish an
    // endpoint.  Reuse that immutable process identity as the owner epoch so a
    // pre-endpoint daemon failure has the same terminal key as the spawn
    // receipt.  The supervisor removes stale terminal receipts before every
    // spawn, so PID reuse cannot admit an earlier owner.
    let owner_epoch = u64::from(std::process::id());
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
use agent_semantic_client_db::runtime_server_control::{read_endpoint, read_supervisor_endpoint};

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_lifecycle_cli.rs"]
mod lifecycle_cli_tests;
