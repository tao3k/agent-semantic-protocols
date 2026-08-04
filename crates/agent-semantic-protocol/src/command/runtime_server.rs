use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_control::prewarm_runtime_server_status_memory;
use agent_semantic_client_db::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerOperation,
    WorkspaceDbRegistry, acquire_runtime_server_election, call_runtime_server,
    prepare_runtime_server_endpoint, runtime_server_endpoint_path,
};
use clap::{Args, Command, CommandFactory, Parser, Subcommand};
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
    /// Query resident OpenTelemetry performance receipts without opening Turso.
    Telemetry(TelemetryQueryArgs),
    /// Run the long-lived ASP Runtime Server under the platform supervisor.
    Daemon,
}

#[derive(Debug, Args)]
struct TelemetryQueryArgs {
    #[arg(long)]
    workspace_identity: String,
    #[arg(long)]
    surface: String,
    #[arg(long)]
    stage: String,
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

pub(super) fn block_on_agent_facing_runtime_server_client<F, T>(
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
    let runtime =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()?;
    match runtime.block_on(async move { tokio::time::timeout_at(deadline, future).await }) {
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
pub(super) const RUNTIME_SERVER_SUPERVISOR_BOUNDARY: std::time::Duration =
    std::time::Duration::from_millis(900);

fn runtime_server_supervisor_boundary_error(
    surface: &'static str,
    stage: &'static str,
    elapsed: std::time::Duration,
) -> String {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-supervisor-wall-failure",
        "schemaVersion": "1",
        "state": "unavailable",
        "surface": surface,
        "stage": stage,
        "reasonKind": "runtime-server-supervisor-boundary-exceeded",
        "boundaryMicros": RUNTIME_SERVER_SUPERVISOR_BOUNDARY.as_micros(),
        "elapsedMicros": elapsed.as_micros(),
        "retryAfterMs": 250,
    })
    .to_string()
}

pub(super) fn block_on_runtime_server_supervisor_client<F, T>(
    surface: &'static str,
    stage: &'static str,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let started = std::time::Instant::now();
    let runtime =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()?;
    match runtime.block_on(async move {
        tokio::time::timeout(RUNTIME_SERVER_SUPERVISOR_BOUNDARY, future).await
    }) {
        Ok(result) => result,
        Err(_) => Err(runtime_server_supervisor_boundary_error(
            surface,
            stage,
            started.elapsed(),
        )),
    }
}

pub(super) fn agent_facing_runtime_wait_remaining(
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
        let _ = agent_semantic_client_db::runtime_server_opentelemetry::try_emit_to_runtime(
            &runtime_server_telemetry_socket_path(&state_home),
            &observation,
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

pub(super) fn runtime_server_workspace_session(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    block_on_runtime_server_client(runtime_server_workspace_session_async(project_root))?
}

pub(super) async fn runtime_server_workspace_session_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_admitted_workspace_scope(project_root).await?;
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)).await?;
    Ok(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity,
            canonical_project_root,
        ),
    )
}

pub(super) async fn runtime_server_workspace_session_for_admission_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    runtime_server_workspace_session_for_resolved_admission_async(
        resolved.workspace.workspace_id.to_string(),
        &resolved.workspace.root,
    )
    .await
}

pub(super) async fn runtime_server_workspace_session_for_resolved_admission_async(
    workspace_identity: impl Into<String>,
    workspace_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    let canonical_project_root =
        tokio::fs::canonicalize(workspace_root)
            .await
            .map_err(|error| {
                format!(
                    "failed to canonicalize Runtime Server admission root {}: {error}",
                    workspace_root.display()
                )
            })?;
    let state_home = state_home()?;
    let endpoint = read_endpoint(&runtime_server_endpoint_path(&state_home)).await?;
    Ok(
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
            &endpoint,
            workspace_identity.into(),
            canonical_project_root,
        ),
    )
}

pub(crate) fn runtime_server_hook_evaluation_client(
    project_root: &Path,
    arguments: Vec<String>,
    input: String,
) -> Result<String, String> {
    let project_root = project_root.to_path_buf();
    block_on_runtime_server_client(async move {
        let session = runtime_server_workspace_session_for_admission_async(&project_root).await?;
        session.evaluate_hook(arguments, input).await
    })?
}

async fn runtime_server_admitted_workspace_scope(
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

pub(super) async fn runtime_server_workspace_generation_client_async(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient,
    String,
> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_admitted_workspace_scope(project_root).await?;
    let state_home = state_home()?;
    // The generation mmap is a data-plane artifact with a canonical state
    // location. Endpoint discovery belongs only to control-plane IPC; reading
    // and validating it here adds unrelated I/O and couples every query lease
    // to daemon transport metadata.
    let workspace_store_root = state_home.join("runtime").join("server").join("workspaces");
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &workspace_store_root,
            &workspace_identity,
            &canonical_project_root,
        )?;
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneOpen;
    let open = agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient::open_state(
        &pointer_path,
    )
    .await?;
    match open {
        WorkspaceGenerationDataPlaneOpen::Ready(client) => Ok(client),
        WorkspaceGenerationDataPlaneOpen::Missing => Err(format!(
            "resident workspace generation is unavailable: reasonKind=active-workspace-generation-required workspace={} workspaceIdentity={workspace_identity}",
            project_root.display()
        )),
        WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } => Err(format!(
            "resident workspace generation requires supervisor reconciliation: reasonKind=active-workspace-generation-reconciliation-required workspace={} workspaceIdentity={workspace_identity} reason={reason}",
            project_root.display()
        )),
    }
}

/// Open the immutable exact-projection segment for the currently published
/// workspace generation.
///
/// This is the resident query data plane: it follows the atomic generation
/// pointer and memory-maps the exact segment without contacting either the
/// Runtime Server control socket or a workspace writer lane. Recovery remains
/// a separate control-plane operation.
pub(super) async fn runtime_server_workspace_exact_projection_client_async(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::runtime_server_workspace::WorkspaceExactProjectionDataPlaneClient,
    String,
> {
    let (workspace_identity, canonical_project_root) =
        runtime_server_admitted_workspace_scope(project_root).await?;
    let state_home = state_home()?;
    let workspace_store_root = state_home.join("runtime").join("server").join("workspaces");
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &workspace_store_root,
            &workspace_identity,
            &canonical_project_root,
        )?;
    use agent_semantic_client_db::runtime_server_workspace::{
        WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
    };
    match WorkspaceExactProjectionDataPlaneClient::open_state(&pointer_path).await? {
        WorkspaceExactProjectionDataPlaneOpen::Ready(client) => Ok(client),
        WorkspaceExactProjectionDataPlaneOpen::Missing => Err(format!(
            "resident workspace exact generation is unavailable: reasonKind=active-workspace-generation-required workspace={} workspaceIdentity={workspace_identity}",
            project_root.display()
        )),
        WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { reason } => Err(format!(
            "resident workspace exact generation requires supervisor reconciliation: reasonKind=active-workspace-generation-reconciliation-required workspace={} workspaceIdentity={workspace_identity} reason={reason}",
            project_root.display()
        )),
    }
}

pub(super) fn runtime_server_current_source_index_snapshot_from_client(
    client: &agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDataPlaneClient,
) -> Result<agent_semantic_client::source_index::CurrentSourceIndexSnapshot, String> {
    let lease = client.lease();
    lease.generation().validate()?;
    let workspace_generation =
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            lease.generation().workspace_generation.clone(),
        )
        .map_err(|error| {
            format!("resident workspace generation evidence is incomplete: {error}")
        })?;
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        lease.generation().owners.iter().map(|owner| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::from(owner.owner_path.as_str()),
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
                    ServerCommand::Telemetry(args) => run_telemetry_query(args).await,
                    ServerCommand::Daemon => unreachable!("daemon handled before client runtime"),
                }
            })?
        }
    }
}

async fn run_telemetry_query(args: TelemetryQueryArgs) -> Result<(), String> {
    let state_home = state_home()?;
    let query =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceQuery::new(
            args.workspace_identity,
            args.surface,
            args.stage,
        );
    let receipt =
        agent_semantic_client_db::runtime_server_opentelemetry::query_runtime_performance(
            &runtime_server_telemetry_query_socket_path(&state_home),
            &query,
        )
        .await?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to encode telemetry query receipt: {error}"))?
    );
    Ok(())
}

async fn run_control(operation: RuntimeServerOperation) -> Result<(), String> {
    let state_home = state_home()?;
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    if operation == RuntimeServerOperation::Reconcile {
        let receipt =
            super::runtime_server_supervisor::reconcile_healthy_runtime_server(&state_home).await?;
        super::protocol_binary::prune_runtime_binary_artifacts(
            &state_home.join("runtime").join("artifacts"),
        )?;
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
            let runtime_artifact_digest =
                super::protocol_binary::canonical_protocol_binary_artifact_digest(
                    &runtime_artifact_path,
                )
                .await?;
            let receipt = RuntimeServerControlReceipt::starting(
                request_id,
                runtime_artifact_digest,
                artifact_catalog.mode_label().to_owned(),
                artifact_catalog.digest(),
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
        super::protocol_binary::canonical_protocol_binary_artifact_digest(
            &state_home.join("runtime").join("bin").join("asp"),
        )
        .await?
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
                super::protocol_binary::canonical_protocol_binary_artifact_digest(
                    &state_home.join("runtime").join("bin").join("asp"),
                )
                .await?,
                artifact_catalog.mode_label().to_owned(),
                artifact_catalog.digest(),
                format!("platform supervisor recovered an unreachable Runtime Server: {error}"),
            )
        }
        Err(error) => return Err(error),
    };
    print_receipt(&receipt).await
}

pub(super) async fn await_healthy_runtime_server(
    state_home: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let started = tokio::time::Instant::now();
    let deadline = started + RUNTIME_SERVER_SUPERVISOR_BOUNDARY;
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
                "platform supervisor did not publish a healthy Runtime Server within {}ms: {}",
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
        super::protocol_binary::canonical_protocol_binary_artifact_digest(&canonical_runtime)
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
pub(crate) async fn probe_healthy_runtime_server_at(state_home: &Path) -> Result<bool, String> {
    let endpoint = read_supervisor_endpoint(&runtime_server_endpoint_path(state_home)).await?;
    prewarm_runtime_server_status_memory(&endpoint).await?;
    let receipt = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_artifact_digest.clone(),
        request_identity("agent-session-runtime-probe").await?,
    )
    .await?;
    Ok(receipt.state
        == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        && tokio::net::UnixStream::connect(runtime_server_telemetry_query_socket_path(state_home))
            .await
            .is_ok())
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

#[path = "graph_turbo_daemon.rs"]
mod graph_turbo_daemon;

use super::runtime_server_supervisor;

#[path = "runtime_server_singleton_socket.rs"]
mod singleton_socket;

async fn run_daemon() -> Result<(), String> {
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let election = acquire_runtime_server_election()
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let state_home = state_home()?;
    let _singleton_socket = match singleton_socket::acquire(&state_home)? {
        singleton_socket::SingletonSocketElection::Acquired(guard) => guard,
        singleton_socket::SingletonSocketElection::ResidentExists => return Ok(()),
    };
    let workspace_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store(
            &state_home.join("runtime").join("server"),
        )
        .await
        .map_err(|error| format!("failed to prepare Runtime Server workspace store: {error}"))?;
    ensure_runtime_protocol_binary_user_path_alias(&state_home)?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest =
        super::protocol_binary::canonical_protocol_binary_artifact_digest(&runtime_artifact_path)
            .await?;
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let endpoint = prepare_runtime_server_endpoint(
        &runtime_artifact_path,
        &runtime_artifact_digest,
        artifact_catalog.mode_label(),
        &artifact_catalog.digest(),
        owner_epoch,
        &binding_token,
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let telemetry_socket_path = runtime_server_telemetry_socket_path(&state_home);
    let telemetry_query_socket_path = runtime_server_telemetry_query_socket_path(&state_home);
    remove_stale_socket(&socket_path).await?;
    remove_stale_socket(&data_plane_socket_path).await?;
    remove_stale_socket(&telemetry_socket_path).await?;
    remove_stale_socket(&telemetry_query_socket_path).await?;

    let admission_catalog =
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            state_home
                .join("runtime")
                .join("server")
                .join("workspace-admissions.v1.json"),
        )
        .await?;
    let agent_session_registry_owner = std::sync::Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            &state_home,
        )
        .await?,
    );
    let (diagnostic_events, diagnostics) =
        agent_semantic_client_db::runtime_server_diagnostics::RuntimeServerDiagnostics::start(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-diagnostic.v1.json"),
        )
        .await?;
    let opentelemetry =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry::start(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-telemetry.turso"),
            telemetry_socket_path.clone(),
            telemetry_query_socket_path.clone(),
        )?;
    let provider_catalog_generation =
        super::global_provider_catalog::read_global_provider_catalog_readiness()?
            .catalog_generation;
    let generation_builder_catalog = provider_catalog_generation.clone();
    let generation_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder =
        std::sync::Arc::new(move |_workspace_identity, project_root| {
            let provider_catalog_generation = generation_builder_catalog.clone();
            Box::pin(async move {
                let (registry, current_catalog_generation) =
                    super::global_provider_catalog::runtime_provider_registry_snapshot(
                        &project_root,
                    )?;
                if current_catalog_generation != provider_catalog_generation {
                    return Err(format!(
                        "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                        provider_catalog_generation, current_catalog_generation
                    ));
                }
                let mut build = agent_semantic_client::source_index::
                    prepare_runtime_server_workspace_generation_with_registry_async(
                        project_root,
                        registry,
                    )
                    .await?;
                build.materialization.provider_schema_digest = current_catalog_generation;
                Ok(build)
            })
        });
    let owner_projection_builder: agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerProjectionBuilder =
        std::sync::Arc::new(|workspace_identity, language_id, project_root, owner| {
            Box::pin(async move {
                let owner_path = owner.owner_path.clone();
                super::provider_resident_exact::build_resident_owner_projection(
                    &workspace_identity,
                    &language_id,
                    &project_root,
                    &owner_path,
                    owner,
                )
                .await
            })
        });
    let hook_evaluation_builder: agent_semantic_client_db::runtime_server::HookEvaluationBuilder =
        std::sync::Arc::new(|_workspace_identity, project_root, arguments, input| {
            Box::pin(async move {
                tokio::task::spawn_blocking(move || {
                    super::hook_runtime::run_protocol_hook_capture_for_project(
                        &project_root,
                        arguments,
                        input,
                    )
                })
                .await
                .map_err(|error| format!("resident hook evaluator task failed: {error}"))?
            })
        });
    let server = RuntimeServer::bind_and_publish_with_artifact_catalog(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        &runtime_server_endpoint_path(&state_home),
        workspace_store,
        std::sync::Arc::new(artifact_catalog),
    )
    .await
    .map_err(|error| format!("failed to bind and publish Runtime Server: {error}"))?
    .with_event_sender(diagnostic_events)
    .with_workspace_generation_builder_catalog_identity(
        generation_builder,
        admission_catalog,
        provider_catalog_generation,
    )
    .with_workspace_owner_projection_builder(owner_projection_builder)
    .with_hook_evaluation_builder(hook_evaluation_builder)
    .with_agent_session_registry_owner(agent_session_registry_owner);
    let mut graph_turbo =
        graph_turbo_daemon::GraphTurboDaemon::start_from_environment(&state_home).await;
    let server = server.with_graph_turbo_resident_status(graph_turbo.status());
    let server = match graph_turbo.evaluation_builder() {
        Some(builder) => server.with_graph_turbo_evaluation_builder(builder),
        None => server,
    };
    let result = server.serve().await.map(|_| ());
    let result = match (result, graph_turbo.shutdown().await) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(server_error), Err(graph_turbo_error)) => Err(format!(
            "Runtime Server failed and Graph Turbo did not drain: server={server_error}; graphTurbo={graph_turbo_error}"
        )),
    };
    let result = match (result, opentelemetry.shutdown().await) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(server_error), Err(telemetry_error)) => Err(format!(
            "Runtime Server failed and OpenTelemetry did not drain: server={server_error}; telemetry={telemetry_error}"
        )),
    };
    let diagnostic_result = diagnostics.join().await;
    cleanup_endpoint(&state_home, &endpoint).await;
    let _ = tokio::fs::remove_file(&telemetry_socket_path).await;
    let _ = tokio::fs::remove_file(&telemetry_query_socket_path).await;
    drop(election);
    match (result, diagnostic_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(server_error), Err(diagnostic_error)) => Err(format!(
            "Runtime Server failed and diagnostic lane did not drain: server={server_error}; diagnostics={diagnostic_error}"
        )),
    }
}

fn ensure_runtime_protocol_binary_user_path_alias(protocol_home: &Path) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is required to establish the ASP runtime PATH entry".to_owned())?;
    let alias = home
        .join(".local/bin")
        .join(super::protocol_binary::SEMANTIC_AGENT_PROTOCOL_BIN);
    super::protocol_binary::ensure_runtime_protocol_binary_alias(protocol_home, &alias)?;
    Ok(alias)
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

fn runtime_server_telemetry_socket_path(state_home: &Path) -> PathBuf {
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

async fn read_supervisor_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate_supervisor_control()?;
    Ok(endpoint)
}

async fn read_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let endpoint = read_supervisor_endpoint(path).await?;
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
