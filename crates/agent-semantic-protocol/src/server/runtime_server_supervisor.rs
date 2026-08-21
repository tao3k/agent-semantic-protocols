//! Process-local lifecycle authority for the single ASP Server owned by one
//! ASP State Home.
//!
//! The daemon is deliberately detached from the invoking CLI/Hook process.
//! Singleton ownership remains daemon-owned through the global Unix socket;
//! no platform service manager or workspace-scoped daemon participates.

use std::path::{Path, PathBuf};
use agent_semantic_client_db::runtime_server_runtime::RUNTIME_SERVER_CONNECTION_IO_BUDGET;

const SPAWN_RECEIPT_FILE: &str = "owner-spawn.v1.json";

pub(crate) use agent_semantic_client_db::RuntimeServerSpawnReceipt;

fn server_dir(protocol_home: &Path) -> PathBuf {
    protocol_home.join("runtime").join("server")
}

fn spawn_receipt_path(protocol_home: &Path) -> PathBuf {
    server_dir(protocol_home).join(SPAWN_RECEIPT_FILE)
}

pub(crate) async fn create_runtime_server_run_intent(protocol_home: &Path) -> Result<(), String> {
    agent_semantic_client_db::runtime_server_lifecycle::create_run_intent(protocol_home).await
}

pub(crate) async fn remove_runtime_server_run_intent(protocol_home: &Path) -> Result<(), String> {
    agent_semantic_client_db::runtime_server_lifecycle::remove_run_intent(protocol_home).await
}

pub(crate) async fn mark_runtime_server_operator_stopped(
    protocol_home: &Path,
) -> Result<(), String> {
    agent_semantic_client_db::runtime_server_lifecycle::mark_operator_stopped(protocol_home).await
}

async fn clear_runtime_server_operator_stop(protocol_home: &Path) -> Result<(), String> {
    agent_semantic_client_db::runtime_server_lifecycle::clear_operator_stopped(protocol_home).await
}

async fn require_runtime_server_not_operator_stopped(protocol_home: &Path) -> Result<(), String> {
    if agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(protocol_home).await? {
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
    crate::prepare_runtime_server_provider_catalog(protocol_home).await?;
    let desired_runtime_identity =
        agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
            protocol_home,
            "asp",
        )
        .await?;
    let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(protocol_home)?;
    match super::runtime_server_endpoint_io::read_supervisor_endpoint(&endpoint_path).await {
        Ok(endpoint) => {
        let status = agent_semantic_client_db::call_runtime_server(
            &endpoint,
            agent_semantic_client_db::RuntimeServerOperation::Status,
            endpoint.runtime_binary_identity.clone(),
            spawn_nonce(),
        )
        .await;
        if let Ok(receipt) = status {
            if receipt.state
                == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
            {
                if endpoint.runtime_binary_identity == desired_runtime_identity.identity() {
                    return Ok(None);
                }
                request_runtime_server_drain(protocol_home).await?;
                agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(
                    protocol_home,
                    endpoint.owner_epoch,
                )
                .await?;
                super::runtime_server_endpoint_io::cleanup_endpoint(protocol_home, &endpoint)
                    .await?;
            }
        }
        }
        Err(_) => {
            // A current-schema decode failure is a stale generation.  Preserve a
            // live owner, but remove an owned dead endpoint so the next spawn can
            // publish the current identity atomically.
            let live_owner = read_runtime_server_spawn_receipt(protocol_home)
                .await?
                .is_some_and(|receipt| receipt.process_id > 0);
            if !live_owner {
                remove_file_if_present(&endpoint_path).await?;
                remove_file_if_present(&spawn_receipt_path(protocol_home)).await?;
            } else {
                let bytes = tokio::fs::read(&endpoint_path).await.map_err(|e| format!("read stale endpoint: {e}"))?;
                let envelope: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("stale endpoint envelope mismatch: {e}"))?;
                let socket = envelope.get("socketPath").and_then(|v|v.as_str()).ok_or_else(||"stale endpoint missing socketPath".to_owned())?;
                let token = envelope.get("bindingToken").and_then(|v|v.as_str()).ok_or_else(||"stale endpoint missing bindingToken".to_owned())?;
                let epoch = envelope.get("ownerEpoch").and_then(|v|v.as_u64()).ok_or_else(||"stale endpoint missing ownerEpoch".to_owned())?;
                let transport = envelope.get("transportContractDigest").and_then(|v|v.as_str()).ok_or_else(||"stale endpoint missing transportContractDigest".to_owned())?;
                let digest = envelope.get("runtimeArtifactDigest").and_then(|v|v.as_str()).ok_or_else(||"stale endpoint missing legacy runtimeArtifactDigest".to_owned())?;
                tokio::time::timeout(
                    RUNTIME_SERVER_CONNECTION_IO_BUDGET,
                    agent_semantic_client_db::runtime_server_control::drain_previous_generation(
                        std::path::Path::new(socket), token, epoch, transport, digest, spawn_nonce(),
                    ),
                )
                .await
                .map_err(|_| "previous generation drain timed out".to_owned())??;
                agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(protocol_home, epoch).await?;
                remove_file_if_present(&endpoint_path).await?;
                remove_file_if_present(&spawn_receipt_path(protocol_home)).await?;
            }
        }
    }
    // The endpoint is deliberately published only after the daemon has bound
    // its IPC listener.  During that interval every CLI/hook caller observes
    // the same missing endpoint.  A published spawn receipt is therefore the
    // durable single-flight authority for that interval: spawning another
    // daemon only contends for the election lock and creates avoidable I/O.
    // A terminal owner receipt invalidates that authority, so the next caller
    // can make exactly one replacement attempt.
    if agent_semantic_client_db::runtime_server_lifecycle::read_latest_owner_exit(protocol_home)
        .await?
        .is_none()
    {
        if let Some(receipt) = read_runtime_server_spawn_receipt(protocol_home).await? {
            if agent_semantic_runtime::runtime_process_lifecycle::process_id_is_alive(receipt.process_id).await {
                return Ok(None);
            }
        }
    }
    spawn_detached_runtime_server(protocol_home).await.map(Some)
}

pub(crate) async fn spawn_detached_runtime_server(
    protocol_home: &Path,
) -> Result<RuntimeServerSpawnReceipt, String> {
    agent_semantic_client_db::runtime_server_lifecycle::remove_stale(protocol_home).await?;
    let runtime_artifact = canonical_supervisor_runtime_artifact(protocol_home).await?;
    let nonce = spawn_nonce();
    let stderr_path = server_dir(protocol_home).join("owner-stderr.log");
    let child = agent_semantic_runtime::runtime_process_lifecycle::launch_detached(
        agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec {
            program: runtime_artifact.clone(), args: vec!["server".to_owned(), "daemon".to_owned()], current_dir: None, environment: vec![("ASP_STATE_HOME".to_owned(), protocol_home.to_string_lossy().into_owned())], stderr: stderr_path,
        }).await?;
    let receipt = RuntimeServerSpawnReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
        schema_version: "1".to_owned(),
        process_id: child.process_id,
        nonce,
        state_home: protocol_home.to_string_lossy().into_owned(),
        runtime_artifact_path: runtime_artifact.to_string_lossy().into_owned(),
    };
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(protocol_home, &receipt).await?;
    Ok(receipt)
}

pub(crate) async fn read_runtime_server_spawn_receipt(
    protocol_home: &Path,
) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(protocol_home).await
}

pub(crate) async fn request_runtime_server_drain(protocol_home: &Path) -> Result<(), String> {
    let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(protocol_home)?;
    let endpoint =
        super::runtime_server_endpoint_io::read_supervisor_endpoint(&endpoint_path).await?;
    let receipt = agent_semantic_client_db::call_runtime_server(
        &endpoint,
        agent_semantic_client_db::RuntimeServerOperation::Restart,
        endpoint.runtime_binary_identity.clone(),
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

pub(crate) async fn escalate_drained_owner(
    protocol_home: &Path,
    owner_epoch: u64,
) -> Result<(), String> {
    let receipt = read_runtime_server_spawn_receipt(protocol_home)
        .await?
        .ok_or_else(|| "graceful drain timeout has no owner receipt".to_owned())?;
    let coordinator = agent_semantic_client_db::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(protocol_home, &receipt.runtime_artifact_path);
    if receipt.state_home != protocol_home.to_string_lossy()
        || coordinator.classify(Some(receipt.process_id)).await? != agent_semantic_client_db::runtime_server_lifecycle_coordinator::OwnerClassification::Live
    {
        return Err("graceful drain timeout owner receipt no longer matches".to_owned());
    }
    coordinator.terminate_verified(receipt.process_id, false).await?;
    let _ = owner_epoch;
    Ok(())
}

pub(crate) async fn force_kill_previous_owner(
    protocol_home: &Path,
    owner_epoch: u64,
) -> Result<(), String> {
    let receipt = read_runtime_server_spawn_receipt(protocol_home)
        .await?
        .ok_or_else(|| "previous owner force-kill has no owner receipt".to_owned())?;
    let coordinator = agent_semantic_client_db::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(protocol_home, &receipt.runtime_artifact_path);
    if receipt.state_home != protocol_home.to_string_lossy()
        || coordinator.classify(Some(receipt.process_id)).await? != agent_semantic_client_db::runtime_server_lifecycle_coordinator::OwnerClassification::Live
    {
        return Err("previous owner force-kill owner identity mismatch".to_owned());
    }
    let _ = owner_epoch;
    coordinator.terminate_verified(receipt.process_id, true).await?;
    Ok(())
}

pub(crate) async fn unload_runtime_server_supervisor(protocol_home: &Path) -> Result<(), String> {
    agent_semantic_client_db::runtime_server_lifecycle::remove_owner_receipt(protocol_home).await
}

pub(crate) async fn reconcile_healthy_runtime_server(
    protocol_home: &Path,
) -> Result<agent_semantic_client_db::runtime_server_control::RuntimeServerControlReceipt, String> {
    ensure_runtime_server(protocol_home, false).await?;
    super::runtime_server::await_healthy_runtime_server_after_spawn().await?;
    super::runtime_server::observe_runtime_server_readiness(protocol_home).await
}

async fn canonical_supervisor_runtime_artifact(protocol_home: &Path) -> Result<PathBuf, String> {
    let stable_entry = protocol_home.join("runtime").join("bin").join("asp");
    let resolved = agent_semantic_runtime::runtime_process_lifecycle::canonicalize(&stable_entry).await.map_err(|error| {
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
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id(),
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
