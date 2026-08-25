//! Runtime process identity receipts consumed by the Runtime supervisor.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use agent_semantic_artifacts::runtime_artifact_catalog::{
    RuntimeArtifactPublication, RuntimeBinaryIdentity, runtime_artifact_source_generation,
};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_executable_identity: Option<RuntimeExecutableIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_executable_identity: Option<RuntimeExecutableIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExecutableIdentity {
    pub path: PathBuf,
    pub dev: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_sec: i64,
    pub mtime_ns: u32,
    pub ctime_sec: i64,
    pub ctime_ns: u32,
    pub content_digest: String,
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
        let generation = match self.source_generation_algorithm.as_str() {
            "filesystem-generation-v1" => runtime_artifact_source_generation(&canonical_source)?,
            _ => {
                return Err(format!(
                    "unknown Runtime artifact source generation algorithm: {}",
                    self.source_generation_algorithm
                ));
            }
        };
        Ok(canonical_source == self.source_path && generation == self.source_generation)
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
}

/// Admit an invoker only when it is the receipt-covered source/active executable.
/// This performs metadata and digest-addressed-path checks; it never reads the binary bytes.
pub fn admit_runtime_invoker(
    current_exe: &Path,
    receipt: &RuntimeArtifactIdentityReceipt,
    active_path: &Path,
) -> Result<(), String> {
    let Some(source_identity) = receipt.source_executable_identity.as_ref() else {
        return Err("state=invoker-artifact-not-active reasonKind=invoker-artifact-not-active repair=asp install binary".to_owned());
    };
    let Some(active_identity) = receipt.active_executable_identity.as_ref() else {
        return Err("state=invoker-artifact-not-active reasonKind=invoker-artifact-not-active repair=asp install binary".to_owned());
    };
    let active = std::fs::canonicalize(active_path).map_err(|error| format!("state=invoker-artifact-not-active reasonKind=active-artifact-unavailable path={} error={error}", active_path.display()))?;
    let active_digest = agent_semantic_content_identity::blake3_digest_from_canonical_artifact_path(&active)
        .ok_or_else(|| "state=invoker-artifact-not-active reasonKind=active-artifact-not-digest-addressed repair=asp install binary".to_owned())?;
    if active_digest != receipt.artifact_digest || active_digest != active_identity.content_digest {
        return Err(format!(
            "state=invoker-artifact-not-active reasonKind=active-artifact-digest-drift expected={} actual={} repair=asp install binary",
            receipt.artifact_digest, active_digest
        ));
    }
    let current = std::fs::canonicalize(current_exe).map_err(|error| format!("state=invoker-artifact-not-active reasonKind=invoker-path-unavailable error={error} repair=asp install binary"))?;
    let metadata = std::fs::metadata(&current).map_err(|error| format!("state=invoker-artifact-not-active reasonKind=invoker-metadata-unavailable error={error} repair=asp install binary"))?;
    let matches = |identity: &RuntimeExecutableIdentity| {
        current == identity.path
            && metadata.len() == identity.size
            && executable_metadata_matches(&metadata, identity)
    };
    let source_matches = matches(source_identity);
    let active_matches = matches(active_identity);
    if !source_matches && !active_matches {
        return Err(format!(
            "state=invoker-artifact-not-active reasonKind=invoker-artifact-not-active currentExecutable={} sourceExecutable={} activeExecutable={} sourceMetadataMatches={} activeMetadataMatches={} repair=asp install binary",
            current.display(),
            source_identity.path.display(),
            active_identity.path.display(),
            source_matches,
            active_matches,
        ));
    }
    Ok(())
}

fn executable_metadata_matches(
    metadata: &std::fs::Metadata,
    identity: &RuntimeExecutableIdentity,
) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        metadata.dev() == identity.dev
            && metadata.ino() == identity.inode
            && metadata.mtime() == identity.mtime_sec
            && metadata.mtime_nsec() as u32 == identity.mtime_ns
            && metadata.ctime() == identity.ctime_sec
            && metadata.ctime_nsec() as u32 == identity.ctime_ns
    }
    #[cfg(not(unix))]
    {
        true
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
    let identity_kind = publication.identity.kind();
    let identity_value = publication
        .identity
        .content_digest()
        .content_digest()
        .as_str()
        .to_owned();
    let identity_algorithm = publication.identity.algorithm();
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind,
        artifact_mode: artifact_mode.to_owned(),
        stable_path: stable_path.to_path_buf(),
        source_path: publication.source_path.clone(),
        source_generation: publication.source_generation.clone(),
        source_generation_algorithm: publication.source_generation_algorithm.clone(),
        artifact_digest: publication
            .artifact_digest
            .content_digest()
            .as_str()
            .to_owned(),
        identity_kind: identity_kind.to_owned(),
        identity_value,
        identity_algorithm: identity_algorithm.to_owned(),
        source_executable_identity: executable_identity(
            &publication.source_path,
            publication.artifact_digest.content_digest().as_str(),
        )
        .ok(),
        active_executable_identity: executable_identity(
            &publication.path,
            publication.artifact_digest.content_digest().as_str(),
        )
        .ok(),
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

fn executable_identity(
    path: &Path,
    content_digest: &str,
) -> Result<RuntimeExecutableIdentity, String> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        format!(
            "canonicalize executable identity {}: {error}",
            path.display()
        )
    })?;
    let metadata = std::fs::metadata(&canonical).map_err(|error| {
        format!(
            "metadata executable identity {}: {error}",
            canonical.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        return Ok(RuntimeExecutableIdentity {
            path: canonical,
            dev: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            mtime_sec: metadata.mtime(),
            mtime_ns: metadata.mtime_nsec() as u32,
            ctime_sec: metadata.ctime(),
            ctime_ns: metadata.ctime_nsec() as u32,
            content_digest: content_digest.to_owned(),
        });
    }
    #[cfg(not(unix))]
    {
        let modified = metadata
            .modified()
            .map_err(|error| format!("mtime executable identity: {error}"))?;
        let nanos = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("mtime executable identity: {error}"))?
            .as_nanos();
        Ok(RuntimeExecutableIdentity {
            path: canonical,
            dev: 0,
            inode: 0,
            size: metadata.len(),
            mtime_sec: (nanos / 1_000_000_000) as i64,
            mtime_ns: (nanos % 1_000_000_000) as u32,
            ctime_sec: 0,
            ctime_ns: 0,
            content_digest: content_digest.to_owned(),
        })
    }
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
        || !matches!(
            receipt.source_generation_algorithm.as_str(),
            "filesystem-generation-v1"
        )
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
        RuntimeArtifactIdentityReceipt, SCHEMA_ID, SCHEMA_VERSION, admit_runtime_invoker,
        executable_identity,
    };
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

    #[test]
    fn invoker_admission_accepts_source_and_active_and_rejects_rebuild() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("source-asp");
        std::fs::write(&source, b"runtime").expect("source");
        let digest = blake3::hash(b"runtime").to_hex().to_string();
        let active = root
            .path()
            .join("runtime/artifacts/blake3-256")
            .join(&digest)
            .join("asp");
        std::fs::create_dir_all(active.parent().expect("active parent")).expect("mkdir");
        std::fs::copy(&source, &active).expect("active");
        let receipt = RuntimeArtifactIdentityReceipt {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            artifact_kind: "asp".to_owned(),
            artifact_mode: "dev".to_owned(),
            stable_path: root.path().join("runtime/bin/asp"),
            source_path: source.canonicalize().expect("canonical source"),
            source_generation: runtime_artifact_source_generation(&source).expect("generation"),
            source_generation_algorithm: "filesystem-generation-v1".to_owned(),
            artifact_digest: digest.clone(),
            identity_kind: "content".to_owned(),
            identity_value: digest.clone(),
            identity_algorithm: "blake3-256".to_owned(),
            source_executable_identity: Some(
                executable_identity(&source, &digest).expect("source identity"),
            ),
            active_executable_identity: Some(
                executable_identity(&active, &digest).expect("active identity"),
            ),
        };
        assert!(admit_runtime_invoker(&source, &receipt, &active).is_ok());
        assert!(admit_runtime_invoker(&active, &receipt, &active).is_ok());
        std::fs::write(&source, b"rebuilt").expect("rebuild");
        assert!(admit_runtime_invoker(&source, &receipt, &active).is_err());
    }

    #[test]
    fn invoker_admission_rejects_missing_identity_and_active_digest_drift() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("source-asp");
        std::fs::write(&source, b"runtime").expect("source");
        let digest = blake3::hash(b"runtime").to_hex().to_string();
        let active = root
            .path()
            .join("runtime/artifacts/blake3-256")
            .join(&digest)
            .join("asp");
        std::fs::create_dir_all(active.parent().expect("active parent")).expect("mkdir");
        std::fs::copy(&source, &active).expect("active");
        let mut receipt = RuntimeArtifactIdentityReceipt {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            artifact_kind: "asp".to_owned(),
            artifact_mode: "dev".to_owned(),
            stable_path: root.path().join("runtime/bin/asp"),
            source_path: source.clone(),
            source_generation: runtime_artifact_source_generation(&source).expect("generation"),
            source_generation_algorithm: "filesystem-generation-v1".to_owned(),
            artifact_digest: digest.clone(),
            identity_kind: "content".to_owned(),
            identity_value: digest.clone(),
            identity_algorithm: "blake3-256".to_owned(),
            source_executable_identity: None,
            active_executable_identity: None,
        };
        assert!(
            admit_runtime_invoker(&source, &receipt, &active)
                .expect_err("missing identity")
                .contains("invoker-artifact-not-active")
        );
        receipt.source_executable_identity =
            Some(executable_identity(&source, &digest).expect("source identity"));
        receipt.active_executable_identity =
            Some(executable_identity(&active, &digest).expect("active identity"));
        std::fs::remove_file(&active).expect("remove active");
        let drift = root
            .path()
            .join("runtime/artifacts/blake3-256")
            .join("a".repeat(64))
            .join("asp");
        std::fs::create_dir_all(drift.parent().expect("drift parent")).expect("mkdir");
        std::fs::copy(&source, &drift).expect("drift");
        assert!(
            admit_runtime_invoker(&source, &receipt, &drift)
                .expect_err("digest drift")
                .contains("active-artifact-digest-drift")
        );
    }

    #[test]
    fn unknown_source_generation_algorithm_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("source");
        std::fs::write(&source, b"runtime").expect("source");
        let receipt = RuntimeArtifactIdentityReceipt {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            artifact_kind: "asp".to_owned(),
            artifact_mode: "dev".to_owned(),
            stable_path: root.path().join("stable"),
            source_path: source.clone(),
            source_generation: "x".to_owned(),
            source_generation_algorithm: "unknown-v1".to_owned(),
            artifact_digest: "a".repeat(64),
            identity_kind: "content".to_owned(),
            identity_value: "a".repeat(64),
            identity_algorithm: "blake3-256".to_owned(),
            source_executable_identity: None,
            active_executable_identity: None,
        };
        assert!(
            receipt
                .source_generation_matches(&source)
                .expect_err("unknown algorithm")
                .contains("unknown Runtime artifact source generation algorithm")
        );
    }
}
pub async fn publish_resident_runtime_artifact_identity(
    state_home: &Path,
    stable_path: &Path,
    source_path: &Path,
    artifact_digest: &str,
    artifact_mode: &str,
) -> Result<RuntimeArtifactIdentityReceipt, String> {
    let artifact_kind = stable_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            format!(
                "resident Runtime artifact stable path has no binary identity: {}",
                stable_path.display()
            )
        })?
        .to_owned();
    if artifact_mode != "dev" && artifact_mode != "release" {
        return Err(format!(
            "resident Runtime artifact mode must be dev or release: {artifact_mode}"
        ));
    }
    let source_path = std::fs::canonicalize(source_path).map_err(|error| {
        format!(
            "resolve resident Runtime artifact source {}: {error}",
            source_path.display()
        )
    })?;
    let source_generation = runtime_artifact_source_generation(&source_path)?;
    let receipt = RuntimeArtifactIdentityReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        artifact_kind,
        artifact_mode: artifact_mode.to_owned(),
        stable_path: stable_path.to_path_buf(),
        source_path: source_path.clone(),
        source_generation,
        source_generation_algorithm: "filesystem-generation-v1".to_owned(),
        artifact_digest: artifact_digest.to_owned(),
        identity_kind: "content".to_owned(),
        identity_value: artifact_digest.to_owned(),
        identity_algorithm: "blake3-256".to_owned(),
        source_executable_identity: executable_identity(&source_path, artifact_digest).ok(),
        active_executable_identity: executable_identity(stable_path, artifact_digest).ok(),
    };
    let receipt_path = runtime_artifact_identity_path(state_home, &receipt.artifact_kind)?;
    let receipt_parent = receipt_path.parent().ok_or_else(|| {
        format!(
            "resident Runtime artifact identity path has no parent: {}",
            receipt_path.display()
        )
    })?;
    tokio::fs::create_dir_all(receipt_parent)
        .await
        .map_err(|error| {
            format!(
                "create resident Runtime artifact identity directory {}: {error}",
                receipt_parent.display()
            )
        })?;
    let stage = receipt_parent.join(format!(
        ".{}.resident-stage-{}-{}",
        receipt.artifact_kind,
        crate::runtime_process_lifecycle::current_process_id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize resident Runtime artifact identity: {error}"))?;
    tokio::fs::write(&stage, bytes).await.map_err(|error| {
        format!(
            "write staged resident Runtime artifact identity {}: {error}",
            stage.display()
        )
    })?;
    if let Err(error) = tokio::fs::rename(&stage, &receipt_path).await {
        let _ = tokio::fs::remove_file(&stage).await;
        return Err(format!(
            "publish resident Runtime artifact identity {}: {error}",
            receipt_path.display()
        ));
    }
    Ok(receipt)
}
