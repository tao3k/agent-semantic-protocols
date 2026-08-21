//! Runtime Server lifecycle receipt store owned by client-db composition.

use std::path::{Path, PathBuf};
use crate::{RuntimeServerDrainReceipt, RuntimeServerExitReceipt, RuntimeServerSpawnReceipt};

const SERVER_DIR: &str = "runtime/server";
fn server_dir(home: &Path) -> PathBuf { home.join(SERVER_DIR) }
fn marker(home: &Path, name: &str) -> PathBuf { server_dir(home).join(name) }

async fn atomic_empty(path: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "lifecycle marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!("stage-{}", agent_semantic_runtime::runtime_process_lifecycle::current_process_id()));
    let file = tokio::fs::File::create(&staged).await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    tokio::fs::rename(staged, path).await.map_err(|e| e.to_string())
}

pub async fn create_run_intent(home: &Path) -> Result<(), String> { atomic_empty(&marker(home, "run-intent.v1")).await }
pub async fn remove_run_intent(home: &Path) -> Result<(), String> { remove_if_present(&marker(home, "run-intent.v1")).await }

pub async fn mark_operator_stopped(home: &Path) -> Result<(), String> {
    let path = marker(home, "operator-stop.v1.json");
    let parent = path.parent().ok_or_else(|| "operator marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!("stage-{}", agent_semantic_runtime::runtime_process_lifecycle::current_process_id()));
    let bytes = serde_json::to_vec(&serde_json::json!({"schemaId":"agent.semantic-protocols.runtime-server-operator-stop","schemaVersion":"1","state":"stopped"})).map_err(|e| e.to_string())?;
    tokio::fs::write(&staged, bytes).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path).await.map_err(|e| e.to_string())
}

pub async fn clear_operator_stopped(home: &Path) -> Result<(), String> { remove_if_present(&marker(home, "operator-stop.v1.json")).await }
pub async fn operator_stopped(home: &Path) -> Result<bool, String> { tokio::fs::try_exists(marker(home, "operator-stop.v1.json")).await.map_err(|e| e.to_string()) }

fn owner_receipt(home: &Path) -> PathBuf { marker(home, "owner-spawn.v1.json") }
pub async fn read_owner_receipt(home: &Path) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    match tokio::fs::read(owner_receipt(home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub async fn write_owner_receipt(home: &Path, receipt: &RuntimeServerSpawnReceipt) -> Result<(), String> {
    let path = owner_receipt(home);
    let parent = path.parent().ok_or_else(|| "owner receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!("stage-{}", agent_semantic_runtime::runtime_process_lifecycle::current_process_id()));
    tokio::fs::write(&staged, serde_json::to_vec(receipt).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path).await.map_err(|e| e.to_string())
}
pub async fn remove_owner_receipt(home: &Path) -> Result<(), String> { remove_if_present(&owner_receipt(home)).await }

fn exit_receipt(home: &Path) -> PathBuf { marker(home, "daemon-exit.v1.json") }
fn drain_receipt(home: &Path) -> PathBuf { marker(home, "daemon-drain.v1.json") }
pub async fn publish_drain(home: &Path, receipt: RuntimeServerDrainReceipt) -> Result<(), String> {
    let path = drain_receipt(home); let parent = path.parent().ok_or_else(|| "drain receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!("stage-{}", agent_semantic_runtime::runtime_process_lifecycle::current_process_id()));
    tokio::fs::write(&staged, serde_json::to_vec(&receipt).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path).await.map_err(|e| e.to_string())
}
pub async fn publish_with_errors(home: &Path, owner_epoch: u64, clean_drain: bool, errors: Vec<String>) -> Result<(), String> {
    let receipt = RuntimeServerExitReceipt { schema_id: "agent.semantic-protocols.runtime-server-daemon-exit".to_owned(), schema_version: "1".to_owned(), owner_epoch, clean_drain, errors };
    let path = exit_receipt(home); let parent = path.parent().ok_or_else(|| "exit receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!("stage-{}", agent_semantic_runtime::runtime_process_lifecycle::current_process_id()));
    tokio::fs::write(&staged, serde_json::to_vec(&receipt).map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path).await.map_err(|e| e.to_string())
}
pub async fn read_latest_owner_exit(home: &Path) -> Result<Option<RuntimeServerExitReceipt>, String> {
    match tokio::fs::read(exit_receipt(home)).await { Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| e.to_string()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None), Err(e) => Err(e.to_string()) }
}
pub async fn remove_stale(home: &Path) -> Result<(), String> { remove_if_present(&exit_receipt(home)).await }
pub async fn await_owner_exit(home: &Path, owner_epoch: u64) -> Result<RuntimeServerExitReceipt, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop { if let Some(receipt) = read_latest_owner_exit(home).await? { if receipt.owner_epoch == owner_epoch { return Ok(receipt); } } if tokio::time::Instant::now() >= deadline { return Err("Runtime Server owner exit receipt timeout".to_owned()); } tokio::time::sleep(std::time::Duration::from_millis(5)).await; }
}

async fn remove_if_present(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await { Ok(()) => Ok(()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), Err(e) => Err(e.to_string()) }
}
