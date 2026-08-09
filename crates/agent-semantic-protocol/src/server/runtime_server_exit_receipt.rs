//! Daemon-owned proof that every Runtime Server service reached a terminal state.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const EXIT_RECEIPT_FILE: &str = "daemon-exit.v1.json";
const DRAIN_RECEIPT_FILE: &str = "daemon-drain.v1.json";
const EXIT_RECEIPT_WAIT_BOUNDARY: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeServerExitReceipt {
    schema_id: String,
    schema_version: String,
    pub(crate) owner_epoch: u64,
    pub(crate) clean_drain: bool,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
}

fn receipt_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join(EXIT_RECEIPT_FILE)
}

fn drain_receipt_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join(DRAIN_RECEIPT_FILE)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeServerDrainReceipt {
    pub(crate) owner_epoch: u64,
    pub(crate) services: serde_json::Value,
    pub(crate) remaining_task_count: usize,
    pub(crate) remaining_child_count: usize,
    pub(crate) clean_drain: bool,
}

pub(crate) async fn publish_drain(
    state_home: &Path,
    receipt: RuntimeServerDrainReceipt,
) -> Result<(), String> {
    let path = drain_receipt_path(state_home);
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime Server drain receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staged = path.with_extension(format!("json.stage-{}", std::process::id()));
    let mut bytes = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-server-drain-receipt",
        "schemaVersion": "1",
        "ownerEpoch": receipt.owner_epoch,
        "services": receipt.services,
        "remainingTaskCount": receipt.remaining_task_count,
        "remainingChildCount": receipt.remaining_child_count,
        "cleanDrain": receipt.clean_drain,
    }))
    .map_err(|error| format!("encode Runtime Server drain receipt: {error}"))?;
    bytes.push(b'\n');
    tokio::fs::write(&staged, bytes)
        .await
        .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
    tokio::fs::rename(&staged, &path)
        .await
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

pub(crate) async fn remove_stale(state_home: &Path) -> Result<(), String> {
    let path = receipt_path(state_home);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale {}: {error}",
            path.display()
        )),
    }
}

/// Synchronous spawn uses the same cleanup authority as the async control
/// path.  A terminal receipt is scoped to exactly one detached-owner attempt.
pub(crate) fn remove_stale_sync(state_home: &Path) -> Result<(), String> {
    let path = receipt_path(state_home);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale {}: {error}",
            path.display()
        )),
    }
}

pub(crate) async fn publish_with_errors(
    state_home: &Path,
    owner_epoch: u64,
    clean_drain: bool,
    errors: Vec<String>,
) -> Result<(), String> {
    let path = receipt_path(state_home);
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime Server exit receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staged = path.with_extension(format!("json.stage-{}", std::process::id()));
    let receipt = RuntimeServerExitReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-daemon-exit".to_owned(),
        schema_version: "1".to_owned(),
        owner_epoch,
        clean_drain,
        errors,
    };
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt)
            .map_err(|error| format!("encode Runtime Server exit receipt: {error}"))?,
    )
    .await
    .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
    tokio::fs::rename(&staged, &path)
        .await
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

pub(crate) async fn await_owner_exit(
    state_home: &Path,
    owner_epoch: u64,
) -> Result<RuntimeServerExitReceipt, String> {
    let path = receipt_path(state_home);
    let started = tokio::time::Instant::now();
    loop {
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let receipt: RuntimeServerExitReceipt = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("decode {}: {error}", path.display()))?;
                if receipt.owner_epoch == owner_epoch {
                    return Ok(receipt);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
        }
        if started.elapsed() >= EXIT_RECEIPT_WAIT_BOUNDARY {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-daemon-exit-failure",
                "schemaVersion": "1",
                "state": "unavailable",
                "reasonKind": "daemon-exit-receipt-boundary-exceeded",
                "ownerEpoch": owner_epoch,
                "boundaryMicros": EXIT_RECEIPT_WAIT_BOUNDARY.as_micros(),
                "elapsedMicros": started.elapsed().as_micros(),
            })
            .to_string());
        }
        tokio::task::yield_now().await;
    }
}

/// Reads an already-published terminal receipt without waiting.  Supervisor
/// reconciliation must not spend its health budget polling an owner that is
/// still alive; it only needs to surface a daemon which has already exited.
pub(crate) async fn read_latest_owner_exit(
    state_home: &Path,
) -> Result<Option<RuntimeServerExitReceipt>, String> {
    let path = receipt_path(state_home);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let receipt: RuntimeServerExitReceipt = serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode {}: {error}", path.display()))?;
            Ok(Some(receipt))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}
