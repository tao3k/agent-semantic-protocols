use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::runtime_artifact_catalog::{RuntimeArtifactPublication, RuntimeBinaryIdentity};

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-identity";
const SCHEMA_VERSION: &str = "1";
static STAGE_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactIdentityReceipt {
    schema_id: String,
    schema_version: String,
    artifact_kind: String,
    artifact_mode: String,
    stable_path: PathBuf,
    source_path: PathBuf,
    artifact_digest: String,
    identity_kind: String,
    identity_value: String,
    identity_algorithm: String,
}

impl RuntimeArtifactIdentityReceipt {
    #[must_use]
    pub fn artifact_kind(&self) -> &str {
        &self.artifact_kind
    }

    #[must_use]
    pub fn artifact_mode(&self) -> &str {
        &self.artifact_mode
    }

    #[must_use]
    pub fn stable_path(&self) -> &Path {
        &self.stable_path
    }

    #[must_use]
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    #[must_use]
    pub fn artifact_digest(&self) -> &str {
        &self.artifact_digest
    }

    pub fn identity_kind(&self) -> &str {
        &self.identity_kind
    }
    pub fn identity_value(&self) -> &str {
        &self.identity_value
    }
    pub fn identity_algorithm(&self) -> &str {
        &self.identity_algorithm
    }

    pub fn identity(&self) -> RuntimeBinaryIdentity {
        match self.identity_kind.as_str() {
            "developer-source-generation" => RuntimeBinaryIdentity::DeveloperSourceGeneration {
                value: self.identity_value.clone(),
                algorithm: self.identity_algorithm.clone(),
            },
            _ => RuntimeBinaryIdentity::Content {
                value: self.identity_value.clone(),
                algorithm: self.identity_algorithm.clone(),
            },
        }
    }
}

pub async fn publish_runtime_artifact_identity(
    state_home: &Path,
    stable_path: &Path,
    publication: &RuntimeArtifactPublication,
) -> Result<RuntimeArtifactIdentityReceipt, String> {
    let artifact_kind = stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            format!(
                "Runtime artifact stable path has no binary identity: {}",
                stable_path.display()
            )
        })?
        .to_owned();
    let artifact_mode = if publication.reference.checkout_root.is_some() {
        "dev"
    } else {
        "release"
    };
    let (identity_kind, identity_value, identity_algorithm) = match &publication.identity {
        RuntimeBinaryIdentity::Content { value, algorithm } => {
            ("content", value.clone(), algorithm.clone())
        }
        RuntimeBinaryIdentity::DeveloperSourceGeneration { value, algorithm } => (
            "developer-source-generation",
            value.clone(),
            algorithm.clone(),
        ),
    };
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind,
        artifact_mode: artifact_mode.to_owned(),
        stable_path: stable_path.to_path_buf(),
        source_path: publication.reference.executable_path.clone(),
        artifact_digest: publication.artifact_digest.clone(),
        identity_kind: identity_kind.to_owned(),
        identity_value,
        identity_algorithm,
    };

    let receipt_path = runtime_artifact_identity_path(state_home, &receipt.artifact_kind)?;
    let receipt_parent = receipt_path.parent().ok_or_else(|| {
        format!(
            "Runtime artifact identity path has no parent: {}",
            receipt_path.display()
        )
    })?;
    tokio::fs::create_dir_all(receipt_parent)
        .await
        .map_err(|error| {
            format!(
                "create Runtime artifact identity directory {}: {error}",
                receipt_parent.display()
            )
        })?;
    let stage = receipt_parent.join(format!(
        ".{}.stage-{}-{}",
        receipt.artifact_kind,
        crate::runtime_process_lifecycle::current_process_id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize Runtime artifact identity: {error}"))?;
    tokio::fs::write(&stage, bytes).await.map_err(|error| {
        format!(
            "write staged Runtime artifact identity {}: {error}",
            stage.display()
        )
    })?;
    if let Err(error) = tokio::fs::rename(&stage, &receipt_path).await {
        let _ = tokio::fs::remove_file(&stage).await;
        return Err(format!(
            "publish Runtime artifact identity {}: {error}",
            receipt_path.display()
        ));
    }
    Ok(receipt)
}

pub async fn read_runtime_artifact_identity(
    state_home: &Path,
    artifact_kind: &str,
) -> Result<RuntimeArtifactIdentityReceipt, String> {
    let receipt_path = runtime_artifact_identity_path(state_home, artifact_kind)?;
    let bytes = tokio::fs::read(&receipt_path).await.map_err(|error| {
        format!(
            "read Runtime artifact identity {}: {error}",
            receipt_path.display()
        )
    })?;
    let receipt: RuntimeArtifactIdentityReceipt =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "parse Runtime artifact identity {}: {error}",
                receipt_path.display()
            )
        })?;
    if receipt.schema_id != SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.artifact_kind != artifact_kind
    {
        return Err(format!(
            "Runtime artifact identity contract mismatch: path={} schemaId={} schemaVersion={} artifactKind={}",
            receipt_path.display(),
            receipt.schema_id,
            receipt.schema_version,
            receipt.artifact_kind
        ));
    }
    let expected_stable_path = state_home.join("runtime/bin").join(artifact_kind);
    if receipt.stable_path != expected_stable_path {
        return Err(format!(
            "Runtime artifact identity stable path mismatch: expected={} actual={}",
            expected_stable_path.display(),
            receipt.stable_path.display()
        ));
    }
    Ok(receipt)
}

fn runtime_artifact_identity_path(
    state_home: &Path,
    artifact_kind: &str,
) -> Result<PathBuf, String> {
    if artifact_kind.is_empty()
        || artifact_kind.contains('/')
        || artifact_kind.contains('\\')
        || artifact_kind == "."
        || artifact_kind == ".."
    {
        return Err(format!(
            "invalid Runtime artifact identity kind: {artifact_kind:?}"
        ));
    }
    Ok(state_home
        .join("runtime/artifact-identities")
        .join(format!("{artifact_kind}.json")))
}

#[cfg(test)]
mod tests {
    use super::{read_runtime_artifact_identity, runtime_artifact_identity_path};

    #[test]
    fn identity_path_is_version_neutral_and_binary_scoped() {
        let state_home = std::path::Path::new("/state");
        assert_eq!(
            runtime_artifact_identity_path(state_home, "asp").expect("identity path"),
            state_home.join("runtime/artifact-identities/asp.json")
        );
        assert!(runtime_artifact_identity_path(state_home, "../asp").is_err());
    }

    #[tokio::test]
    async fn rejects_identity_with_wrong_schema_version() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = runtime_artifact_identity_path(root.path(), "asp").expect("identity path");
        tokio::fs::create_dir_all(path.parent().expect("identity parent"))
            .await
            .expect("create identity parent");
        tokio::fs::write(
            &path,
            br#"{"schemaId":"agent.semantic-protocols.runtime-artifact-identity","schemaVersion":"2","artifactKind":"asp","artifactMode":"dev","stablePath":"/state/runtime/bin/asp","sourcePath":"/checkout/target/debug/asp","artifactDigest":"digest"}"#,
        )
        .await
        .expect("write invalid identity");
        assert!(
            read_runtime_artifact_identity(root.path(), "asp")
                .await
                .is_err()
        );
    }
}
