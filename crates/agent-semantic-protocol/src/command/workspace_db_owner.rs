//! Lifecycle for the resident Turso service scoped to one workspace.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(super) fn run_workspace_db_command(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("resident")
        || args.get(1).map(String::as_str) != Some("serve")
    {
        return Err(usage());
    }
    let (workspace, runtime_binary_digest) = workspace_arg(&args[2..])?;
    serve(&workspace, &runtime_binary_digest)
}

fn workspace_arg(args: &[String]) -> Result<(PathBuf, String), String> {
    let mut workspace = None;
    let mut runtime_binary_digest = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--workspace" if workspace.is_none() => {
                workspace = Some(args.get(index + 1).ok_or_else(usage).map(PathBuf::from)?);
            }
            "--runtime-binary-digest" if runtime_binary_digest.is_none() => {
                runtime_binary_digest = Some(args.get(index + 1).ok_or_else(usage)?.clone());
            }
            _ => return Err(usage()),
        }
        index += 2;
    }
    Ok((
        workspace.ok_or_else(usage)?,
        runtime_binary_digest.ok_or_else(usage)?,
    ))
}

use super::workspace_db_checkpoint::{ResidentServiceCheckpointDecision, checkpoint_decision};

const DEFAULT_RESIDENT_SERVICE_CHECKPOINT_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(60);
const DEFAULT_RESIDENT_SERVICE_IDLE_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(60 * 60);

fn configured_duration(environment_key: &str, default: std::time::Duration) -> std::time::Duration {
    std::env::var(environment_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(std::time::Duration::from_millis)
        .unwrap_or(default)
}

fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn cleanup_owned_endpoint(
    endpoint_path: &Path,
    endpoint: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerEndpoint,
) -> Result<(), String> {
    match std::fs::read(endpoint_path) {
        Ok(bytes) => {
            let current: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerEndpoint =
                serde_json::from_slice(&bytes).map_err(|error| {
                    format!(
                        "failed to decode workspace DB owner endpoint during cleanup {}: {error}",
                        endpoint_path.display()
                    )
                })?;
            if current.workspace_identity == endpoint.workspace_identity
                && current.owner_epoch == endpoint.owner_epoch
                && current.binding_token == endpoint.binding_token
                && current.socket_path == endpoint.socket_path
            {
                std::fs::remove_file(endpoint_path).map_err(|error| {
                    format!(
                        "failed to remove workspace DB owner endpoint {}: {error}",
                        endpoint_path.display()
                    )
                })?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect workspace DB owner endpoint during cleanup {}: {error}",
                endpoint_path.display()
            ));
        }
    }
    match std::fs::remove_file(&endpoint.socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove workspace DB owner socket {}: {error}",
            endpoint.socket_path
        )),
    }
}

fn serve(workspace: &Path, runtime_binary_digest: &str) -> Result<(), String> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(workspace)?;
    let runtime_base =
        agent_semantic_client_db::workspace_db_ipc::workspace_db_owner_runtime_base();
    let Some(owner_election) =
        agent_semantic_client_db::workspace_db_ipc::try_acquire_workspace_db_owner_election(
            &runtime_base,
            resolved.workspace.workspace_id.as_str(),
        )?
    else {
        eprintln!(
            "[workspace-resident-service-existing] workspaceIdentity={}",
            resolved.workspace.workspace_id
        );
        return Ok(());
    };
    let runtime = super::workspace_db_runtime::handle()?;
    let bootstrap_started = std::time::Instant::now();
    let registry = std::sync::Arc::new(agent_semantic_client_db::WorkspaceDbRegistry::default());

    let owner_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("workspace DB owner clock is before Unix epoch: {error}"))?
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX);
    let binding_token = agent_semantic_content_identity::ArtifactHash::blake3(
        format!(
            "{}\0{}\0{}",
            resolved.workspace.workspace_id,
            std::process::id(),
            owner_epoch
        )
        .as_bytes(),
    )
    .value;
    let runtime_binary_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve resident runtime binary: {error}"))?;
    let endpoint = agent_semantic_client_db::workspace_db_ipc::prepare_workspace_db_owner_endpoint(
        &runtime_base,
        resolved.workspace.workspace_id.as_str(),
        owner_epoch,
        std::process::id(),
        &runtime_binary_path,
        runtime_binary_digest,
        &binding_token,
    )?;
    agent_semantic_client_db::workspace_db_ipc::remove_stale_workspace_db_owner_socket(
        &owner_election,
        &endpoint,
    )?;
    let endpoint_path = resolved
        .paths
        .hooks_dir
        .join("state")
        .join("workspace-db-resident-service-endpoint.v1.json");
    let serve_result = runtime.block_on(async {
        let listener =
            agent_semantic_client_db::workspace_db_ipc::bind_workspace_db_owner(&endpoint)?;
        publish_endpoint(&endpoint_path, &endpoint)?;
        let bootstrap_elapsed = bootstrap_started.elapsed();
    println!(
        "[workspace-resident-service-ready] workspaceIdentity={} ownerEpoch={} runtimeBinaryDigest={} transportContractDigest={} endpoint={}",
        endpoint.workspace_identity,
        endpoint.owner_epoch,
        endpoint.runtime_binary_digest,
        endpoint.transport_contract_digest,
        endpoint_path.display()
        );
        std::io::stdout()
            .flush()
            .map_err(|error| format!("failed to flush workspace DB readiness receipt: {error}"))?;
        eprintln!(
            "[workspace-resident-service-ready] workspaceIdentity={} ownerEpoch={} runtimeBinaryDigest={} transportContractDigest={} endpoint={} socket={} bootstrapMicros={} databaseOpens=0 schemaBootstraps=0",
            endpoint.workspace_identity,
            endpoint.owner_epoch,
            endpoint.runtime_binary_digest,
            endpoint.transport_contract_digest,
            endpoint_path.display(),
            endpoint.socket_path,
            bootstrap_elapsed.as_micros(),
        );
        std::io::stderr()
            .flush()
            .map_err(|error| format!("failed to flush workspace DB owner receipt: {error}"))?;
        let last_activity_epoch_seconds =
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(now_epoch_seconds()));
        let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(false);
        let checkpoint_workspace_root = resolved.workspace.root.clone();
        let checkpoint_workspace_identity = endpoint.workspace_identity.clone();
        let checkpoint_activity = std::sync::Arc::clone(&last_activity_epoch_seconds);
        let checkpoint_registry = std::sync::Arc::clone(&registry);
        let checkpoint_interval = configured_duration(
            "ASP_WORKSPACE_RESIDENT_SERVICE_CHECKPOINT_INTERVAL_MS",
            DEFAULT_RESIDENT_SERVICE_CHECKPOINT_INTERVAL,
        );
        let idle_timeout = configured_duration(
            "ASP_WORKSPACE_RESIDENT_SERVICE_IDLE_TIMEOUT_MS",
            DEFAULT_RESIDENT_SERVICE_IDLE_TIMEOUT,
        );
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(checkpoint_interval);
            interval.tick().await;
            loop {
                interval.tick().await;
                let now = now_epoch_seconds();
                let last_activity =
                    checkpoint_activity.load(std::sync::atomic::Ordering::Relaxed);
                let decision = checkpoint_decision(
                    checkpoint_workspace_root.is_dir(),
                    last_activity,
                    now,
                    idle_timeout,
                );
                if decision == ResidentServiceCheckpointDecision::Continue {
                    continue;
                }
                if let Err(error) = checkpoint_registry
                    .finish_loaded_writes(
                        agent_semantic_client_db::WorkspaceDbWriteFinishMode::OwnerDurabilityBoundary,
                    )
                    .await
                {
                    eprintln!(
                        "[workspace-resident-service-checkpoint] status=flush-failed workspaceIdentity={} reason={:?} error={error}",
                        checkpoint_workspace_identity,
                        decision
                    );
                }
                eprintln!(
                    "[workspace-resident-service-checkpoint] status=shutdown workspaceIdentity={} reason={:?} idleSeconds={}",
                    checkpoint_workspace_identity,
                    decision,
                    now.saturating_sub(last_activity)
                );
                let _ = shutdown_sender.send(true);
                break;
            }
        });
        agent_semantic_client_db::workspace_db_ipc::serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity_epoch_seconds,
            shutdown_receiver,
        )
        .await
    });
    let cleanup_result = cleanup_owned_endpoint(&endpoint_path, &endpoint);
    serve_result?;
    cleanup_result
}

fn publish_endpoint(
    path: &Path,
    endpoint: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbOwnerEndpoint,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "workspace DB owner endpoint has no parent: {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create workspace DB owner state directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".workspace-db-resident-service-endpoint.v1.tmp-{}",
        std::process::id()
    ));
    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| format!("failed to encode workspace DB owner endpoint: {error}"))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "failed to stage workspace DB owner endpoint {}: {error}",
                temporary.display()
            )
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("failed to commit workspace DB owner endpoint bytes: {error}"))?;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to protect workspace DB owner endpoint: {error}"))?;
    std::fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to publish workspace DB owner endpoint {}: {error}",
            path.display()
        )
    })?;
    Ok(())
}

fn usage() -> String {
    "usage: asp workspace-db resident serve --workspace <project-root> --runtime-binary-digest <blake3-digest>"
        .to_owned()
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_owner.rs"]
mod tests;
