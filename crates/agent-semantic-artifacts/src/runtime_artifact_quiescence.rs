use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::blake3_content_digest::Blake3ContentDigest;

const QUIESCENCE_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-quiescence";
const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactQuiescenceLease {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: String,
    pub producer_process_id: u32,
    pub lease_nonce: String,
    pub artifact_digest: Blake3ContentDigest,
    pub created_at_unix_millis: u128,
}

#[derive(Debug)]
pub struct PreparedRuntimeArtifactQuiescenceLease {
    pub lease: RuntimeArtifactQuiescenceLease,
    path: PathBuf,
}

impl PreparedRuntimeArtifactQuiescenceLease {
    pub fn consume_under_artifact_guard(&self) -> Result<PathBuf, String> {
        let consumed = self.path.with_file_name(format!(
            "quiescence-consumed-{}-{}.json",
            std::process::id(),
            self.lease.lease_nonce
        ));
        std::fs::rename(&self.path, &consumed).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-consume-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })?;
        Ok(consumed)
    }

    pub fn restore_after_failed_commit(&self, consumed: &Path) -> Result<(), String> {
        std::fs::rename(consumed, &self.path).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-restore-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })
    }

    pub fn finish_consumption(&self, consumed: &Path) -> Result<(), String> {
        std::fs::remove_file(consumed).map_err(|error| {
            format!(
                "reasonKind=runtime-artifact-quiescence-finalize-failed operation={} lease={} error={error}",
                self.lease.operation, self.lease.lease_nonce
            )
        })
    }
}

pub fn runtime_artifact_quiescence_lease_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("leases")
        .join("artifact-publication.v1.json")
}

pub async fn prepare_runtime_artifact_quiescence_lease(
    state_home: &Path,
    operation: &str,
    artifact_digest: &Blake3ContentDigest,
) -> Result<PreparedRuntimeArtifactQuiescenceLease, String> {
    if operation.is_empty() {
        return Err(
            "reasonKind=runtime-artifact-quiescence-identity-incomplete operation or artifact digest is empty"
                .to_owned(),
        );
    }
    let path = runtime_artifact_quiescence_lease_path(state_home);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        let lease: RuntimeArtifactQuiescenceLease = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode Runtime artifact quiescence lease: {error}"))?;
        validate_lease(&lease, operation, artifact_digest)?;
        return Ok(PreparedRuntimeArtifactQuiescenceLease { lease, path });
    }

    let created_at_unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read wall clock for quiescence lease: {error}"))?
        .as_millis();
    let producer_process_id = std::process::id();
    let nonce_material =
        format!("{operation}\0{artifact_digest}\0{producer_process_id}\0{created_at_unix_millis}");
    let lease = RuntimeArtifactQuiescenceLease {
        schema_id: QUIESCENCE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation: operation.to_owned(),
        producer_process_id,
        lease_nonce: blake3::hash(nonce_material.as_bytes()).to_hex().to_string(),
        artifact_digest: artifact_digest.clone(),
        created_at_unix_millis,
    };
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime artifact quiescence lease has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("create Runtime artifact lease directory: {error}"))?;
    let temporary = path.with_file_name(format!(
        ".artifact-publication-{}-{}.tmp",
        producer_process_id, lease.lease_nonce
    ));
    tokio::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&lease)
            .map_err(|error| format!("encode Runtime artifact quiescence lease: {error}"))?,
    )
    .await
    .map_err(|error| format!("stage Runtime artifact quiescence lease: {error}"))?;
    match tokio::fs::rename(&temporary, &path).await {
        Ok(()) => Ok(PreparedRuntimeArtifactQuiescenceLease { lease, path }),
        Err(error) => {
            let _ = tokio::fs::remove_file(&temporary).await;
            Err(format!(
                "reasonKind=runtime-artifact-quiescence-publication-failed operation={operation} error={error}"
            ))
        }
    }
}

fn validate_lease(
    lease: &RuntimeArtifactQuiescenceLease,
    operation: &str,
    artifact_digest: &Blake3ContentDigest,
) -> Result<(), String> {
    if lease.schema_id != QUIESCENCE_SCHEMA_ID
        || lease.schema_version != SCHEMA_VERSION
        || lease.operation != operation
        || &lease.artifact_digest != artifact_digest
        || lease.producer_process_id == 0
        || lease.lease_nonce.is_empty()
    {
        return Err(format!(
            "reasonKind=runtime-artifact-quiescence-identity-mismatch expectedOperation={operation} actualOperation={} expectedArtifactDigest={artifact_digest} actualArtifactDigest={}",
            lease.operation, lease.artifact_digest
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lease_is_consumed_once_by_the_artifact_transaction() {
        let temporary = tempfile::tempdir().expect("temporary state");
        let digest = Blake3ContentDigest::parse(
            "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        )
        .expect("typed digest");
        let prepared =
            prepare_runtime_artifact_quiescence_lease(temporary.path(), "publish:asp", &digest)
                .await
                .expect("prepare lease");
        let consumed = prepared
            .consume_under_artifact_guard()
            .expect("consume lease");
        prepared
            .finish_consumption(&consumed)
            .expect("finish consumption");
        assert!(!runtime_artifact_quiescence_lease_path(temporary.path()).exists());
        assert!(prepared.consume_under_artifact_guard().is_err());
    }

    #[tokio::test]
    async fn mismatched_operation_cannot_consume_existing_lease() {
        let temporary = tempfile::tempdir().expect("temporary state");
        let digest = Blake3ContentDigest::parse(
            "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        )
        .expect("typed digest");
        prepare_runtime_artifact_quiescence_lease(temporary.path(), "publish:asp", &digest)
            .await
            .expect("prepare lease");
        let error =
            prepare_runtime_artifact_quiescence_lease(temporary.path(), "publish:other", &digest)
                .await
                .expect_err("mismatched operation must fail");
        assert!(error.contains("reasonKind=runtime-artifact-quiescence-identity-mismatch"));
    }
}
