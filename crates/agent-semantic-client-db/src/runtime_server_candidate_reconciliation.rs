//! Protocol-neutral candidate reconciliation outcome.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::{read_runtime_server_endpoint, RuntimeServerSpawnReceipt};
use crate::runtime_server_lifecycle::write_owner_receipt;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousCandidatePromotionEnvelope {
    pub desired_identity: String,
    pub executable_path: String,
    pub nonce: String,
    pub process_id: u32,
    pub schema_version: String,
    pub started_at: u64,
    pub state: String,
    pub state_home: String,
}

pub fn validate_promotion_envelope(envelope: &PreviousCandidatePromotionEnvelope, state_home: &Path) -> Result<(), &'static str> {
    if envelope.schema_version != "1" { return Err("candidate-proof-mismatch"); }
    if envelope.state != "promoting" || envelope.started_at == 0 { return Err("candidate-proof-mismatch"); }
    if envelope.desired_identity.is_empty() || envelope.nonce.is_empty() || envelope.executable_path.is_empty() || envelope.process_id == 0 { return Err("candidate-proof-mismatch"); }
    if envelope.state_home != state_home.to_string_lossy() { return Err("candidate-proof-mismatch"); }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReconciliationOutcome {
    pub schema_version: String,
    pub state: String,
    pub reason_kind: String,
    pub error: Option<String>,
    pub candidate_path: Option<String>,
    pub canonical_pid: Option<u64>,
    pub candidate_pid: Option<u64>,
}

impl CandidateReconciliationOutcome {
    pub fn failed(reason_kind: impl Into<String>, error: impl Into<String>) -> Self {
        Self { schema_version: "1".into(), state: "failed".into(), reason_kind: reason_kind.into(), error: Some(error.into()), candidate_path: None, canonical_pid: None, candidate_pid: None }
    }
}

pub async fn prepare_candidate_path(state_home: &Path) -> Result<Option<PathBuf>, String> {
    let directory = state_home.join("runtime/server/candidates");
    let mut entries = match tokio::fs::read_dir(&directory).await { Ok(entries) => entries, Err(_) => return Ok(None) };
    let mut first = None;
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        if first.is_none() { first = Some(entry.path()); }
    }
    Ok(first)
}

pub async fn reconcile_candidates(state_home: &Path) -> Result<CandidateReconciliationOutcome, String> {
    let candidate = prepare_candidate_path(state_home).await?;
    Ok(match candidate {
        Some(path) => CandidateReconciliationOutcome { schema_version: "1".into(), state: "observed".into(), reason_kind: "candidate-present".into(), error: None, candidate_path: Some(path.display().to_string()), canonical_pid: None, candidate_pid: None },
        None => CandidateReconciliationOutcome { schema_version: "1".into(), state: "skipped".into(), reason_kind: "no-candidate".into(), error: None, candidate_path: None, canonical_pid: None, candidate_pid: None },
    })
}

pub async fn repair_canonical_owner_receipt(state_home: &Path) -> Result<CandidateReconciliationOutcome, String> {
    let directory = state_home.join("runtime/server/candidates");
    let mut entries = match tokio::fs::read_dir(&directory).await { Ok(entries) => entries, Err(_) => return Ok(CandidateReconciliationOutcome { schema_version: "1".into(), state: "skipped".into(), reason_kind: "no-promoting-candidate".into(), error: None, candidate_path: None, canonical_pid: None, candidate_pid: None }) };
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
        let envelope: PreviousCandidatePromotionEnvelope = match serde_json::from_slice(&bytes) { Ok(value) => value, Err(_) => continue };
        if envelope.state != "promoting" { continue; }
        let mismatch = || CandidateReconciliationOutcome { schema_version: "1".into(), state: "failed".into(), reason_kind: "candidate-proof-mismatch".into(), error: Some("promoting candidate proof is incomplete".into()), candidate_path: Some(path.display().to_string()), canonical_pid: None, candidate_pid: Some(envelope.process_id as u64) };
        if validate_promotion_envelope(&envelope, state_home).is_err() { return Ok(mismatch()); }
        let endpoint = match read_runtime_server_endpoint(state_home) { Ok(Some(endpoint)) => endpoint, _ => return Ok(mismatch()) };
        if endpoint.runtime_binary_identity.value() != envelope.desired_identity { return Ok(mismatch()); }
        let coordinator = crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(state_home, &envelope.executable_path);
        if coordinator.classify(Some(envelope.process_id)).await.ok() != Some(crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live) { return Ok(mismatch()); }
        let receipt = RuntimeServerSpawnReceipt { schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".into(), schema_version: "1".into(), process_id: envelope.process_id, nonce: envelope.nonce, state_home: envelope.state_home, runtime_artifact_path: envelope.executable_path };
        write_owner_receipt(state_home, &receipt).await?;
        tokio::fs::remove_file(path).await.map_err(|e| e.to_string())?;
        return Ok(CandidateReconciliationOutcome { schema_version: "1".into(), state: "repaired".into(), reason_kind: "promoting-candidate".into(), error: None, candidate_path: None, canonical_pid: None, candidate_pid: Some(receipt.process_id as u64) });
    }
    Ok(CandidateReconciliationOutcome { schema_version: "1".into(), state: "skipped".into(), reason_kind: "no-promoting-candidate".into(), error: None, candidate_path: None, canonical_pid: None, candidate_pid: None })
}
