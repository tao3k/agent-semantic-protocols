//! Process-local lifecycle authority for the single ASP Server owned by one
//! ASP State Home.
//!
//! The daemon is deliberately detached from the invoking CLI/Hook process.
//! Singleton ownership remains daemon-owned through the global Unix socket;
//! no platform service manager or workspace-scoped daemon participates.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;

const OPERATOR_STOP_MARKER_FILE: &str = "operator-stop.v1.json";
const RUN_INTENT_MARKER_FILE: &str = "run-intent.v1";
const SPAWN_RECEIPT_FILE: &str = "owner-spawn.v1.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeServerSpawnReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub process_id: u32,
    pub nonce: String,
    pub state_home: String,
    pub runtime_artifact_path: String,
}

fn server_dir(protocol_home: &Path) -> PathBuf {
    protocol_home.join("runtime").join("server")
}

fn run_intent_marker_path(protocol_home: &Path) -> PathBuf {
    server_dir(protocol_home).join(RUN_INTENT_MARKER_FILE)
}

fn operator_stop_marker_path(protocol_home: &Path) -> PathBuf {
    server_dir(protocol_home).join(OPERATOR_STOP_MARKER_FILE)
}

fn spawn_receipt_path(protocol_home: &Path) -> PathBuf {
    server_dir(protocol_home).join(SPAWN_RECEIPT_FILE)
}

pub(crate) async fn create_runtime_server_run_intent(protocol_home: &Path) -> Result<(), String> {
    let path = run_intent_marker_path(protocol_home);
    let parent = path
        .parent()
        .ok_or_else(|| "run-intent marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("create run-intent parent: {error}"))?;
    let staged = path.with_extension(format!("stage-{}", std::process::id()));
    let file = tokio::fs::File::create(&staged)
        .await
        .map_err(|error| format!("create run-intent: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("sync run-intent: {error}"))?;
    drop(file);
    tokio::fs::rename(&staged, &path)
        .await
        .map_err(|error| format!("publish run-intent: {error}"))?;
    Ok(())
}

pub(crate) async fn remove_runtime_server_run_intent(protocol_home: &Path) -> Result<(), String> {
    remove_file_if_present(&run_intent_marker_path(protocol_home)).await
}

pub(crate) async fn mark_runtime_server_operator_stopped(
    protocol_home: &Path,
) -> Result<(), String> {
    let path = operator_stop_marker_path(protocol_home);
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime Server operator-stop marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staged = path.with_extension(format!("json.stage-{}", std::process::id()));
    let marker = serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-operator-stop",
        "schemaVersion": "1",
        "state": "stopped",
    });
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&marker)
            .map_err(|error| format!("encode Runtime Server operator-stop marker: {error}"))?,
    )
    .await
    .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
    tokio::fs::rename(&staged, &path)
        .await
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

async fn clear_runtime_server_operator_stop(protocol_home: &Path) -> Result<(), String> {
    remove_file_if_present(&operator_stop_marker_path(protocol_home)).await
}

async fn require_runtime_server_not_operator_stopped(protocol_home: &Path) -> Result<(), String> {
    if tokio::fs::try_exists(operator_stop_marker_path(protocol_home))
        .await
        .map_err(|error| format!("inspect Runtime Server operator-stop marker: {error}"))?
    {
        return Err(serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-operator-stop",
            "schemaVersion": "1",
            "state": "stopped",
            "reasonKind": "operator-stop-is-authoritative",
            "nextCommand": "asp server start",
        })
        .to_string());
    }
    Ok(())
}

/// Ensure the global daemon exists. Explicit CLI start/restart clears a prior
/// operator stop; automatic callers never do.
pub(crate) async fn ensure_runtime_server(
    protocol_home: &Path,
    explicit: bool,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    if explicit {
        clear_runtime_server_operator_stop(protocol_home).await?;
    } else {
        require_runtime_server_not_operator_stopped(protocol_home).await?;
    }
    create_runtime_server_run_intent(protocol_home).await?;
    let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(protocol_home);
    if let Ok(endpoint) =
        super::runtime_server_endpoint_io::read_supervisor_endpoint(&endpoint_path).await
    {
        let status = agent_semantic_client_db::call_runtime_server(
            &endpoint,
            agent_semantic_client_db::RuntimeServerOperation::Status,
            endpoint.runtime_artifact_digest.clone(),
            spawn_nonce(),
        )
        .await;
        if matches!(
            status,
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
        ) {
            return Ok(None);
        }
    }
    // The endpoint is deliberately published only after the daemon has bound
    // its IPC listener.  During that interval every CLI/hook caller observes
    // the same missing endpoint.  A published spawn receipt is therefore the
    // durable single-flight authority for that interval: spawning another
    // daemon only contends for the election lock and creates avoidable I/O.
    // A terminal owner receipt invalidates that authority, so the next caller
    // can make exactly one replacement attempt.
    if crate::server::runtime_server_exit_receipt::read_latest_owner_exit(protocol_home)
        .await?
        .is_none()
        && read_runtime_server_spawn_receipt(protocol_home)
            .await?
            .is_some()
    {
        return Ok(None);
    }
    spawn_detached_runtime_server(protocol_home).map(Some)
}

fn spawn_detached_runtime_server(
    protocol_home: &Path,
) -> Result<RuntimeServerSpawnReceipt, String> {
    crate::server::runtime_server_exit_receipt::remove_stale_sync(protocol_home)?;
    let runtime_artifact = canonical_supervisor_runtime_artifact_sync(protocol_home)?;
    let nonce = spawn_nonce();
    let stderr_path = server_dir(protocol_home).join("owner-stderr.log");
    let stderr = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stderr_path)
        .map_err(|error| {
            format!(
                "failed to open Runtime Server stderr receipt {}: {error}",
                stderr_path.display()
            )
        })?;
    let mut command = std::process::Command::new(&runtime_artifact);
    command
        .args(["server", "daemon"])
        .env("ASP_STATE_HOME", protocol_home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn().map_err(|error| {
        format!(
            "failed to detach ASP Runtime Server {}: {error}",
            runtime_artifact.display()
        )
    })?;
    let receipt = RuntimeServerSpawnReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
        schema_version: "1".to_owned(),
        process_id: child.id(),
        nonce,
        state_home: protocol_home.to_string_lossy().into_owned(),
        runtime_artifact_path: runtime_artifact.to_string_lossy().into_owned(),
    };
    let bytes = serde_json::to_vec(&receipt)
        .map_err(|error| format!("encode Runtime Server spawn receipt: {error}"))?;
    std::fs::write(spawn_receipt_path(protocol_home), bytes)
        .map_err(|error| format!("publish Runtime Server spawn receipt: {error}"))?;
    Ok(receipt)
}

pub(crate) async fn read_runtime_server_spawn_receipt(
    protocol_home: &Path,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    match tokio::fs::read(spawn_receipt_path(protocol_home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("decode Runtime Server spawn receipt: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read Runtime Server spawn receipt: {error}")),
    }
}

pub(crate) async fn request_runtime_server_drain(protocol_home: &Path) -> Result<(), String> {
    let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(protocol_home);
    let endpoint =
        super::runtime_server_endpoint_io::read_supervisor_endpoint(&endpoint_path).await?;
    let receipt = agent_semantic_client_db::call_runtime_server(
        &endpoint,
        agent_semantic_client_db::RuntimeServerOperation::Restart,
        endpoint.runtime_artifact_digest.clone(),
        spawn_nonce(),
    )
    .await?;
    if receipt.state
        != agent_semantic_client_db::runtime_server_control::RuntimeServerState::Draining
    {
        return Err("Runtime Server refused graceful drain".to_owned());
    }
    Ok(())
}

pub(crate) async fn unload_runtime_server_supervisor(protocol_home: &Path) -> Result<(), String> {
    remove_file_if_present(&spawn_receipt_path(protocol_home)).await
}

pub(crate) async fn reconcile_healthy_runtime_server(
    protocol_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    ensure_runtime_server(protocol_home, false).await?;
    super::runtime_server::await_healthy_runtime_server(protocol_home).await
}

pub(super) async fn configured_graph_turbo_artifact_at_state_home(
    state_home: &Path,
) -> Result<Option<ConfiguredGraphTurboArtifact>, String> {
    let Some(authority) =
        crate::runtime_artifact::validated_graph_turbo_resident_config(state_home).await?
    else {
        return Ok(None);
    };
    let mut configured = validate_graph_turbo_artifact(
        state_home,
        authority.locator,
        authority.execution_artifact_digest,
    )
    .await?;
    configured.runtime_artifact_digest = authority.runtime_artifact_digest;
    Ok(Some(configured))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConfiguredGraphTurboArtifact {
    pub(super) locator: PathBuf,
    pub(super) runtime_artifact_digest: String,
}

async fn validate_graph_turbo_artifact(
    state_home: &Path,
    locator: PathBuf,
    runtime_artifact_digest: String,
) -> Result<ConfiguredGraphTurboArtifact, String> {
    if !locator.is_absolute() {
        return Err("Graph Turbo executionArtifactLocator must be an absolute path".to_owned());
    }
    if !runtime_artifact_digest.starts_with("blake3-256:") {
        return Err("Graph Turbo runtimeArtifactDigest must use blake3-256".to_owned());
    }
    let canonical = tokio::fs::canonicalize(&locator)
        .await
        .map_err(|error| format!("resolve Graph Turbo execution artifact: {error}"))?;
    let managed_root = tokio::fs::canonicalize(state_home.join("runtime"))
        .await
        .map_err(|error| format!("resolve managed Runtime root: {error}"))?;
    if !canonical.starts_with(&managed_root) {
        return Err(format!(
            "Graph Turbo runtime artifact is outside the managed Runtime root: artifact={} managedRoot={}",
            canonical.display(),
            managed_root.display()
        ));
    }
    let actual_digest = format!(
        "blake3-256:{}",
        crate::command::protocol_binary::canonical_protocol_binary_artifact_digest(&canonical)
            .await?
    );
    if actual_digest != runtime_artifact_digest {
        return Err(format!(
            "Graph Turbo runtime artifact digest drift: expected={runtime_artifact_digest} actual={actual_digest} artifact={}",
            canonical.display()
        ));
    }
    Ok(ConfiguredGraphTurboArtifact {
        locator: canonical,
        runtime_artifact_digest,
    })
}

fn canonical_supervisor_runtime_artifact_sync(protocol_home: &Path) -> Result<PathBuf, String> {
    let stable_entry = protocol_home.join("runtime").join("bin").join("asp");
    let resolved = std::fs::canonicalize(&stable_entry).map_err(|error| {
        format!(
            "canonical ASP Runtime Server binary is unavailable at {}: {error}",
            stable_entry.display()
        )
    })?;
    if !resolved.is_file() {
        return Err(format!(
            "canonical ASP Runtime Server binary target is not a file: {}",
            resolved.display()
        ));
    }
    Ok(stable_entry)
}

fn spawn_nonce() -> String {
    let seed = format!(
        "{}:{}:{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
        std::thread::current().id(),
    );
    format!("blake3-256:{}", blake3::hash(seed.as_bytes()).to_hex())
}

async fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove {}: {error}", path.display())),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_detached_lifecycle.rs"]
mod tests;
