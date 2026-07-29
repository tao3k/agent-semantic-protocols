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
    let workspace = workspace_arg(&args[2..])?;
    serve(&workspace)
}

fn workspace_arg(args: &[String]) -> Result<PathBuf, String> {
    let mut workspace = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] != "--workspace" || workspace.is_some() {
            return Err(usage());
        }
        workspace = Some(args.get(index + 1).ok_or_else(usage).map(PathBuf::from)?);
        index += 2;
    }
    workspace.ok_or_else(usage)
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

fn serve(workspace: &Path) -> Result<(), String> {
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
    let workspace_session = super::workspace_db_runtime::block_on(
        registry.bootstrap_workspace(&resolved.workspace.root),
    )??;
    let bootstrap_elapsed = bootstrap_started.elapsed();

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
    let owner_artifact = std::env::current_exe()
        .map_err(|error| format!("failed to resolve workspace DB owner artifact: {error}"))?;
    let owner_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(&owner_artifact)?.to_string();
    let endpoint = agent_semantic_client_db::workspace_db_ipc::prepare_workspace_db_owner_endpoint(
        &runtime_base,
        resolved.workspace.workspace_id.as_str(),
        &owner_artifact_digest,
        owner_epoch,
        &binding_token,
    )?;
    agent_semantic_client_db::workspace_db_ipc::remove_stale_workspace_db_owner_socket(
        &owner_election,
        &endpoint,
    )?;
    let listener = {
        let _runtime_guard = runtime.enter();
        agent_semantic_client_db::workspace_db_ipc::bind_workspace_db_owner(&endpoint)?
    };
    let endpoint_path = resolved
        .paths
        .hooks_dir
        .join("state")
        .join("workspace-db-resident-service-endpoint.v1.json");
    publish_endpoint(&endpoint_path, &endpoint)?;
    eprintln!(
        "[workspace-resident-service-ready] workspaceIdentity={} ownerEpoch={} endpoint={} socket={} bootstrapMicros={} databaseOpens=1 schemaBootstraps=1",
        endpoint.workspace_identity,
        endpoint.owner_epoch,
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
    let checkpoint_session = workspace_session.clone();
    let checkpoint_interval = configured_duration(
        "ASP_WORKSPACE_RESIDENT_SERVICE_CHECKPOINT_INTERVAL_MS",
        DEFAULT_RESIDENT_SERVICE_CHECKPOINT_INTERVAL,
    );
    let idle_timeout = configured_duration(
        "ASP_WORKSPACE_RESIDENT_SERVICE_IDLE_TIMEOUT_MS",
        DEFAULT_RESIDENT_SERVICE_IDLE_TIMEOUT,
    );
    runtime.spawn(async move {
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
            if let Err(error) = checkpoint_session
                .finish_writes(
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
    let serve_result = super::workspace_db_runtime::block_on(
        agent_semantic_client_db::workspace_db_ipc::serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity_epoch_seconds,
            shutdown_receiver,
        ),
    )?;
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

/*
    use super::{ResidentServiceCheckpointDecision, checkpoint_decision};

    #[test]
    fn existing_active_workspace_continues() {
        assert_eq!(
            checkpoint_decision(true, 1_000, 1_100, std::time::Duration::from_secs(3_600),),
            ResidentServiceCheckpointDecision::Continue
        );
    }

    #[test]
    fn missing_workspace_shuts_down_immediately() {
        assert_eq!(
            checkpoint_decision(false, 1_000, 1_001, std::time::Duration::from_secs(3_600),),
            ResidentServiceCheckpointDecision::ShutdownWorkspaceMissing
        );
    }

    #[test]
    fn one_hour_idle_workspace_shuts_down() {
        assert_eq!(
            checkpoint_decision(true, 1_000, 4_600, std::time::Duration::from_secs(3_600),),
            ResidentServiceCheckpointDecision::ShutdownIdle
        );
    }
*/

fn usage() -> String {
    "usage: asp workspace-db resident serve --workspace <project-root>".to_owned()
}
