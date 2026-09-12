// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::RuntimeServerControlReceipt;
use agent_semantic_client_db::RuntimeServerOperation;
use agent_semantic_client_db::call_runtime_server_for_state_home;
use agent_semantic_client_db::runtime_server_control::prewarm_runtime_server_status_memory;
use clap::Command;
use clap::CommandFactory;
use clap::Parser;
use clap::Subcommand;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[path = "runtime_server_daemon.rs"]
mod runtime_server_daemon;
#[path = "runtime_server_generation_builder.rs"]
mod runtime_server_generation_builder;
#[path = "runtime_server_identity_handoff.rs"]
mod runtime_server_identity_handoff;
#[path = "runtime_server_query_generation_observer.rs"]
mod runtime_server_query_generation_observer;
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
    /// Atomically drain and replace the verified owner from the applied V1 activation.
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

pub(crate) async fn runtime_server_workspace_session_for_admission_async(
    project_root: &Path,
) -> Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession, String> {
    // Multi-Agent v2 registration is a global Runtime control-plane
    // operation. It requires a stable workspace identity, but it must not
    // depend on a source-generation locator, Hook mmap inbox, or admission
    // catalog. Binding through the global endpoint removes the registration ->
    // generation -> typed-agent recursion.
    agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
        project_root,
    )
    .await
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerRestartReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    state: &'static str,
    lifecycle_authority: &'static str,
    publication_nonce: String,
    artifact_digest: String,
    previous_owner_epoch: Option<u64>,
    owner_epoch: u64,
}

async fn run_restart() -> Result<(), String> {
    let state_home = state_home()?;
    let event = operator_start_activation_event(&state_home)
        .await?
        .ok_or_else(|| {
            "state=runtime-server-restart-unavailable reasonKind=activation-event-missing"
                .to_owned()
        })?;
    let previous_owner_epoch =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await?
        .map(|endpoint| endpoint.owner_epoch);
    let healthy =
        super::runtime_server_wire_adapter::restart_healthy_runtime_server_for_activation_event(
            &state_home,
            &event,
        )
        .await?;
    if healthy.state
        != agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
    {
        return Err(
            "state=runtime-server-restart-failed reasonKind=replacement-not-healthy".to_owned(),
        );
    }
    let owner = agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
        .await?
        .ok_or_else(|| {
            "state=runtime-server-restart-failed reasonKind=replacement-owner-missing".to_owned()
        })?;
    let endpoint =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await?
        .ok_or_else(|| {
            "state=runtime-server-restart-failed reasonKind=replacement-endpoint-missing".to_owned()
        })?;
    if endpoint.owner_process_id != owner.process_id {
        return Err(
            "state=runtime-server-restart-failed reasonKind=replacement-owner-binding-mismatch"
                .to_owned(),
        );
    }
    print_receipt(&RuntimeServerRestartReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-restart-receipt.v1",
        schema_version: "1",
        state: "healthy",
        lifecycle_authority: "state-home-supervisor-transaction",
        publication_nonce: event.publication_nonce,
        artifact_digest: event.artifact_digest.to_string(),
        previous_owner_epoch,
        owner_epoch: endpoint.owner_epoch,
    })
    .await
}

async fn run_control_status() -> Result<(), String> {
    let state_home = state_home()?;
    let (resident_transaction, endpoint) = agent_semantic_client_db::runtime_server_lifecycle::
        observe_resident_transaction_with_endpoint(&state_home)
        .await
        .map_err(|error| {
            format!(
                "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime status requires a content-bound Host handoff: {error}"
            )
        })?;
    let _runtime_handoff = crate::AspClientRuntimeHandoff::try_from(&resident_transaction)?;
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
        event.previous_artifact_digest.as_ref(),
    )
    .await?;
    Ok(())
}

async fn operator_start_activation_event(
    state_home: &Path,
) -> Result<
    Option<agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent>,
    String,
> {
    if let Some(event) =
        agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
            state_home,
        )
        .await?
    {
        return Ok(Some(event));
    }
    agent_semantic_artifacts::runtime_artifact_activation::read_applied_runtime_artifact_activation_event(
        state_home,
    )
    .await
}

async fn run_status() -> Result<(), String> {
    match run_control_status().await {
        Ok(()) => Ok(()),
        Err(reason) => Err(status_observation_failure(&reason)),
    }
}

fn status_observation_failure(reason: &str) -> String {
    let lower = reason.to_ascii_lowercase();
    let (reason_kind, failure_layer) =
        if lower.contains("operation not permitted") || lower.contains("os error 1") {
            (
                "host-operation-not-permitted",
                "runtime-verified-endpoint-transport",
            )
        } else {
            (
                "runtime-status-observation-failed",
                "runtime-lifecycle-observation",
            )
        };
    format!(
        "state=blocked failureLayer={failure_layer} reasonKind={reason_kind} operation=status originalError={reason}"
    )
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
    {
        validate_runtime_server_service_publication(&endpoint)
            .await
            .map_err(|error| status_observation_failure(&error))?;
        if let Err(error) =
            agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
                state_home,
            )
            .await
        {
            receipt.state =
                agent_semantic_client_db::runtime_server_control::RuntimeServerState::Starting;
            receipt.reason = Some(format!(
                "Runtime endpoint is authenticated but its resident transaction is not committed: {error}"
            ));
        }
    }
    Ok(receipt)
}

async fn validate_runtime_server_service_publication(
    endpoint: &agent_semantic_client_db::RuntimeServerEndpoint,
) -> Result<(), String> {
    endpoint.validate_service_reachability().await
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

pub(crate) async fn ensure_healthy_runtime_server_for_bounded_operation()
-> Result<RuntimeServerControlReceipt, String> {
    let state_home = state_home()?;
    let event = operator_start_activation_event(&state_home)
        .await?
        .ok_or_else(|| {
            "state=runtime-server-client-bootstrap-failed reasonKind=activation-event-missing"
                .to_owned()
        })?;
    if !agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
        &state_home,
        &event.artifact_digest,
        &event.publication_nonce,
    )
    .await?
    {
        return Err(
            "state=runtime-server-client-bootstrap-failed reasonKind=runtime-server-operator-stopped"
                .to_owned(),
        );
    }
    let receipt =
        super::runtime_server_wire_adapter::ensure_healthy_runtime_server_for_client_recovery(
            &state_home,
            &event,
        )
        .await?;
    if receipt.resident_transaction.is_none() {
        return Err(
            "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime bootstrap returned Healthy without its resident transaction"
                .to_owned(),
        );
    }
    Ok(receipt)
}

/// Establish one content-bound Runtime handoff and admit the exact workspace
/// through the Runtime control plane before any data-plane `ClientFrame` is
/// opened.
///
/// Workspace admission is deliberately not inferred from a Query payload. The
/// canonical project root is resolved by the shared State identity owner and
/// committed by the Runtime's single workspace-admission catalog authority.
pub(crate) async fn ensure_healthy_runtime_server_for_workspace(
    project_root: &Path,
) -> Result<RuntimeServerControlReceipt, String> {
    let mut ready = ensure_healthy_runtime_server_for_bounded_operation().await?;
    let state_home = state_home()?;
    let (observed_transaction, endpoint) = agent_semantic_client_db::runtime_server_lifecycle::
        observe_resident_transaction_with_endpoint(&state_home)
        .await?;
    let ready_transaction = ready.resident_transaction.as_ref().ok_or_else(|| {
        "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime bootstrap returned Healthy without its resident transaction".to_owned()
    })?;
    if observed_transaction.publication_nonce != ready_transaction.publication_nonce
        || observed_transaction.applied_artifact_digest != ready_transaction.applied_artifact_digest
        || observed_transaction.endpoint_owner_epoch != ready_transaction.endpoint_owner_epoch
        || endpoint.runtime_binary_identity.content_digest()
            != &ready_transaction.endpoint_binary_content_digest
    {
        return Err(
            "reasonKind=runtime-workspace-admission-handoff-mismatch failureLayer=runtime-control-admission Runtime workspace admission crossed its content-bound handoff"
                .to_owned(),
        );
    }
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let canonical_root = resolved.workspace.root;
    agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
        &endpoint,
        &canonical_root,
        request_identity("ensure-workspace").await?,
    )
    .await
    .map_err(|error| {
        format!(
            "reasonKind=runtime-workspace-admission-failed failureLayer=runtime-control-admission {error}"
        )
    })?;
    // Preserve the original transaction as the only data-plane capability.
    // The control receipt is an admission acknowledgement, never a replacement
    // Runtime identity.
    ready.resident_transaction = Some(observed_transaction);
    Ok(ready)
}

pub(crate) fn state_home() -> Result<PathBuf, String> {
    agent_semantic_runtime::state_core::resolve_state_home()
}

pub(crate) fn runtime_server_telemetry_socket_path(state_home: &Path) -> Result<PathBuf, String> {
    Ok(agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .transport()
        .opentelemetry_socket())
}

pub(crate) fn runtime_server_telemetry_query_socket_path(
    state_home: &Path,
) -> Result<PathBuf, String> {
    Ok(agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .transport()
        .opentelemetry_query_socket())
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
    // PID is process-location evidence, not resident authority: the OS may
    // reuse it immediately after a dead owner.  Bind replay, cleanup, and
    // endpoint admission to a fresh non-zero epoch instead.  This identity is
    // local to the elected daemon and is not an artifact-publication counter,
    // clock, nonce, or second mutation transaction.
    let owner_epoch = u64::from_be_bytes(
        entropy[..8]
            .try_into()
            .expect("fixed-size OS entropy prefix"),
    ) | 1;
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
use agent_semantic_client_db::runtime_server_control::read_supervisor_endpoint;

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_lifecycle_cli.rs"]
mod lifecycle_cli_tests;
