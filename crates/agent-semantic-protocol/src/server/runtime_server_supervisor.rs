//! Process-local lifecycle authority for the single ASP Server owned by one
//! ASP State Home.
//!
//! The daemon is deliberately detached from the invoking CLI/Hook process.
//! Singleton ownership remains daemon-owned through the global Unix socket;
//! no platform service manager or workspace-scoped daemon participates.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use agent_semantic_client_db::runtime_server_control::{
    RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET, RuntimeServerHookProbeError, RuntimeServerState,
    probe_runtime_server_for_hook, read_runtime_server_endpoint,
    validate_runtime_server_endpoint_for_state_home,
};

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
    spawn_detached_runtime_server(protocol_home).map(Some)
}

/// Hook fast path. The endpoint is discovery only; a bounded control-socket
/// Status exchange is the liveness authority. The daemon election remains the
/// singleton authority when an absent or refused endpoint requires a spawn.
pub(crate) fn ensure_runtime_server_for_hook(protocol_home: &Path) -> Result<(), String> {
    if operator_stop_marker_path(protocol_home).exists() {
        return Ok(());
    }
    let should_spawn = match read_runtime_server_endpoint(protocol_home)? {
        None => true,
        Some(endpoint) => {
            validate_runtime_server_endpoint_for_state_home(protocol_home, &endpoint)?;
            match probe_runtime_server_for_hook(&endpoint, spawn_nonce()) {
                Ok(receipt) if receipt.state == RuntimeServerState::Healthy => false,
                Ok(receipt) => {
                    return Err(runtime_server_hook_transient_error(
                        receipt.state,
                        receipt.reason.as_deref(),
                    ));
                }
                Err(RuntimeServerHookProbeError::Stale(_)) => true,
                Err(RuntimeServerHookProbeError::LiveTransient(reason)) => {
                    return Err(runtime_server_hook_probe_error(
                        "runtime-server-hook-control-live-transient",
                        &reason,
                    ));
                }
                Err(RuntimeServerHookProbeError::FailClosed(reason)) => {
                    return Err(runtime_server_hook_probe_error(
                        "runtime-server-hook-control-identity-failure",
                        &reason,
                    ));
                }
            }
        }
    };
    if !should_spawn {
        return Ok(());
    }
    std::fs::create_dir_all(server_dir(protocol_home))
        .map_err(|error| format!("create Runtime Server state directory: {error}"))?;
    std::fs::write(run_intent_marker_path(protocol_home), [])
        .map_err(|error| format!("create Runtime Server Hook run intent: {error}"))?;
    let _ = spawn_detached_runtime_server(protocol_home)?;
    Ok(())
}

fn runtime_server_hook_transient_error(state: RuntimeServerState, reason: Option<&str>) -> String {
    runtime_server_hook_probe_error(
        "runtime-server-hook-control-live-transient",
        &format!("state={state:?} reason={}", reason.unwrap_or("none")),
    )
}

fn runtime_server_hook_probe_error(reason_kind: &str, reason: &str) -> String {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-hook-liveness-failure.v1",
        "schemaVersion": "1",
        "state": "unavailable",
        "stage": "runtime-server-hook-control-probe",
        "reasonKind": reason_kind,
        "reason": reason,
        "executionBudgetMicros": RUNTIME_SERVER_HOOK_CONTROL_PROBE_BUDGET.as_micros(),
        "retryAfterMs": 100,
    })
    .to_string()
}

fn spawn_detached_runtime_server(
    protocol_home: &Path,
) -> Result<RuntimeServerSpawnReceipt, String> {
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

pub(super) async fn configured_graph_turbo_python_at_state_home(
    state_home: &Path,
    configured: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let config_path = server_dir(state_home).join("graph-turbo-resident-config.v1.json");
    if let Some(configured) = configured {
        let configured = validate_graph_turbo_python(state_home, configured).await?;
        let document = serde_json::json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-config",
            "schemaVersion": "1",
            "pythonExecutionLocator": configured,
        });
        tokio::fs::create_dir_all(server_dir(state_home))
            .await
            .map_err(|error| format!("create Graph Turbo config directory: {error}"))?;
        tokio::fs::write(
            &config_path,
            serde_json::to_vec_pretty(&document)
                .map_err(|error| format!("encode Graph Turbo resident config: {error}"))?,
        )
        .await
        .map_err(|error| format!("write Graph Turbo resident config: {error}"))?;
        return Ok(Some(configured));
    }
    let document = match tokio::fs::read(&config_path).await {
        Ok(document) => document,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read Graph Turbo resident config: {error}")),
    };
    let document: serde_json::Value = serde_json::from_slice(&document)
        .map_err(|error| format!("decode Graph Turbo resident config: {error}"))?;
    let configured = document
        .get("pythonExecutionLocator")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "Graph Turbo resident config locator is missing".to_owned())?;
    validate_graph_turbo_python(state_home, configured)
        .await
        .map(Some)
}

async fn validate_graph_turbo_python(
    state_home: &Path,
    configured: PathBuf,
) -> Result<PathBuf, String> {
    if !configured.is_absolute() {
        return Err("ASP_GRAPH_TURBO_PYTHON must be an absolute path".to_owned());
    }
    let canonical = tokio::fs::canonicalize(&configured)
        .await
        .map_err(|error| format!("resolve Graph Turbo Python artifact: {error}"))?;
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
    Ok(canonical)
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
