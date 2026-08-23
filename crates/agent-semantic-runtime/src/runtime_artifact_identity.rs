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
    source_generation: String,
    source_generation_algorithm: String,
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

    pub fn source_generation_matches(&self, source_path: &Path) -> Result<bool, String> {
        let canonical_source = std::fs::canonicalize(source_path).map_err(|error| {
            format!(
                "resolve Runtime artifact source generation {}: {error}",
                source_path.display()
            )
        })?;
        Ok(
            self.source_generation_algorithm == "filesystem-generation-v1"
                && canonical_source == self.source_path
                && runtime_artifact_source_generation(&canonical_source)? == self.source_generation,
        )
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
        RuntimeBinaryIdentity::Content {
            value: self.identity_value.clone(),
            algorithm: self.identity_algorithm.clone(),
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
    let RuntimeBinaryIdentity::Content { value, algorithm } = &publication.identity;
    let (identity_kind, identity_value, identity_algorithm) =
        ("content", value.clone(), algorithm.clone());
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind,
        artifact_mode: artifact_mode.to_owned(),
        stable_path: stable_path.to_path_buf(),
        source_path: publication.source_path.clone(),
        source_generation: publication.source_generation.clone(),
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
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
    if !matches!(receipt.artifact_mode.as_str(), "dev" | "release")
        || receipt.identity_kind != "content"
        || receipt.source_generation_algorithm != "filesystem-generation-v1"
        || !receipt
            .source_generation
            .strip_prefix("blake3-256:")
            .is_some_and(|value| {
                value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        || receipt.identity_algorithm != "blake3-256"
        || receipt.identity_value != receipt.artifact_digest
        || receipt.artifact_digest.len() != 64
        || !receipt
            .artifact_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "Runtime artifact identity content contract mismatch: path={} mode={} identityKind={} identityAlgorithm={}",
            receipt_path.display(),
            receipt.artifact_mode,
            receipt.identity_kind,
            receipt.identity_algorithm,
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

pub fn runtime_artifact_source_generation(source_path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(source_path).map_err(|error| {
        format!(
            "inspect Runtime artifact source generation {}: {error}",
            source_path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "Runtime artifact source generation is not a file: {}",
            source_path.display()
        ));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.filesystem-generation.v1\0");
    hasher.update(source_path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        hasher.update(&metadata.dev().to_le_bytes());
        hasher.update(&metadata.ino().to_le_bytes());
        hasher.update(&metadata.mtime().to_le_bytes());
        hasher.update(&metadata.mtime_nsec().to_le_bytes());
        hasher.update(&metadata.ctime().to_le_bytes());
        hasher.update(&metadata.ctime_nsec().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        let modified = metadata
            .modified()
            .map_err(|error| format!("read source modification time: {error}"))?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("source modification time predates epoch: {error}"))?;
        hasher.update(&modified.as_nanos().to_le_bytes());
    }
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
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
    use super::{
        read_runtime_artifact_identity, runtime_artifact_identity_path,
        runtime_artifact_source_generation,
    };

    #[test]
    fn identity_path_is_version_neutral_and_binary_scoped() {
        let state_home = std::path::Path::new("/state");
        assert_eq!(
            runtime_artifact_identity_path(state_home, "asp").expect("identity path"),
            state_home.join("runtime/artifact-identities/asp.json")
        );
        assert!(runtime_artifact_identity_path(state_home, "../asp").is_err());
    }

    #[test]
    fn source_generation_is_stable_until_the_source_changes() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("asp");
        std::fs::write(&source, b"generation-a").expect("write first generation");
        let first = runtime_artifact_source_generation(&source).expect("first generation");
        let current = runtime_artifact_source_generation(&source).expect("current generation");
        assert_eq!(first, current);

        std::fs::write(&source, b"generation-b").expect("write second generation");
        let second = runtime_artifact_source_generation(&source).expect("second generation");
        assert_ne!(first, second);
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

    #[tokio::test]
    async fn rejects_non_content_or_mismatched_identity() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = runtime_artifact_identity_path(root.path(), "asp").expect("identity path");
        tokio::fs::create_dir_all(path.parent().expect("identity parent"))
            .await
            .expect("create identity parent");
        let digest = "a".repeat(64);
        tokio::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-artifact-identity",
                "schemaVersion": "1",
                "artifactKind": "asp",
                "artifactMode": "dev",
                "stablePath": root.path().join("runtime/bin/asp"),
                "sourcePath": root.path().join("checkout/target/debug/asp"),
                "sourceGeneration": format!("blake3-256:{}", "b".repeat(64)),
                "sourceGenerationAlgorithm": "filesystem-generation-v1",
                "artifactDigest": digest.clone(),
                "identityKind": "source-generation",
                "identityValue": digest,
                "identityAlgorithm": "metadata"
            }))
            .expect("encode invalid identity"),
        )
        .await
        .expect("write invalid identity");
        let error = read_runtime_artifact_identity(root.path(), "asp")
            .await
            .expect_err("non-content identity must fail closed");
        assert!(error.contains("content contract mismatch"));
    }
}
