use agent_semantic_content_identity::active_artifact_merkle::{
    ActiveArtifactKind, ActiveArtifactLeaf, ActiveArtifactLeafInput, ActiveAspArtifactReceipt,
};
use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;

const ACTIVE_ASP_ARTIFACT_RECEIPT_FILE: &str = "active-asp-artifact-receipt.v1.json";
const ACTIVE_ASP_ARTIFACT_SET_ID: &str = "asp-runtime";

#[derive(Debug, Clone)]
pub struct ActiveAspArtifactMaterialization {
    pub receipt_path: PathBuf,
    pub receipt: ActiveAspArtifactReceipt,
    pub artifact_byte_reads: usize,
    pub artifact_bytes_read: u64,
    pub receipt_writes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveArtifactMetadataFingerprint {
    materialized_path: String,
    size_bytes: u64,
    modified_unix_nanos: u64,
    change_time_unix_nanos: Option<i64>,
}

#[derive(Clone, Debug)]
struct VerifiedActiveAspArtifactReceiptCacheEntry {
    receipt_path: PathBuf,
    receipt_size_bytes: u64,
    receipt_modified_unix_nanos: u64,
    receipt_change_time_unix_nanos: Option<i64>,
    activation_path: PathBuf,
    asp_paths: Vec<PathBuf>,
    leaf_fingerprints: Vec<ActiveArtifactMetadataFingerprint>,
    receipt: ActiveAspArtifactReceipt,
}

static VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE: OnceLock<
    Mutex<Option<VerifiedActiveAspArtifactReceiptCacheEntry>>,
> = OnceLock::new();

pub fn active_asp_artifact_receipt_path(activation_path: &Path) -> Result<PathBuf, String> {
    let parent = activation_path.parent().ok_or_else(|| {
        format!(
            "activation path has no state directory: {}",
            activation_path.display()
        )
    })?;
    Ok(parent.join(ACTIVE_ASP_ARTIFACT_RECEIPT_FILE))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveAspArtifactReconciliation {
    NotMaterialized,
    Current,
    Updated,
}

impl ActiveAspArtifactReconciliation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotMaterialized => "not-materialized",
            Self::Current => "current",
            Self::Updated => "updated",
        }
    }
}

pub fn rebind_active_asp_binary_receipt_if_present(
    binary_path: &Path,
    binary_digest: &str,
    activation_path: &Path,
) -> Result<ActiveAspArtifactReconciliation, String> {
    if !activation_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", activation_path.display()))?
    {
        return Ok(ActiveAspArtifactReconciliation::NotMaterialized);
    }
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    if !receipt_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", receipt_path.display()))?
    {
        return Ok(ActiveAspArtifactReconciliation::NotMaterialized);
    }
    let receipt_bytes = fs::read(&receipt_path).map_err(|error| {
        format!(
            "failed to read active ASP artifact receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    let previous_receipt: ActiveAspArtifactReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| {
            format!(
                "failed to parse active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
    previous_receipt.validate().map_err(|error| {
        format!(
            "invalid active ASP artifact receipt {}: {error:?}",
            receipt_path.display()
        )
    })?;
    let binary_path = canonical_regular_file(binary_path, "ASP binary")?;
    let activation_path = canonical_regular_file(activation_path, "activation")?;
    let binary_metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("failed to inspect {}: {error}", binary_path.display()))?;
    let activation_metadata = fs::metadata(&activation_path)
        .map_err(|error| format!("failed to inspect {}: {error}", activation_path.display()))?;
    let activation_bytes = fs::read(&activation_path)
        .map_err(|error| format!("failed to read {}: {error}", activation_path.display()))?;
    let binary_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(binary_digest)
            .map_err(|_| format!("invalid BLAKE3 ASP binary digest: {binary_digest}"))?
            .into_content_digest();
    let leaves = vec![
        ActiveArtifactLeaf::new(
            ActiveArtifactLeafInput::new(
                "runtime/asp",
                utf8_path(&binary_path, "ASP binary")?,
                ActiveArtifactKind::AspBinary,
                binary_digest,
            )
            .with_materialization_metadata(
                binary_metadata.len(),
                modified_unix_nanos(&binary_metadata)?,
                change_time_unix_nanos(&binary_metadata),
            ),
        )?,
        ActiveArtifactLeaf::new(
            ActiveArtifactLeafInput::new(
                "state/activation.json",
                utf8_path(&activation_path, "activation")?,
                ActiveArtifactKind::Activation,
                blake3_content_digest_v1(&activation_bytes),
            )
            .with_materialization_metadata(
                activation_bytes.len() as u64,
                modified_unix_nanos(&activation_metadata)?,
                change_time_unix_nanos(&activation_metadata),
            ),
        )?,
    ];
    let receipt = ActiveAspArtifactReceipt::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    if receipt == previous_receipt {
        return Ok(ActiveAspArtifactReconciliation::Current);
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write_compare_exchange(&receipt_path, &bytes, Some(&receipt_bytes))?;
    Ok(ActiveAspArtifactReconciliation::Updated)
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_receipt_reconciliation.rs"]
mod active_artifact_receipt_reconciliation_tests;

pub fn materialize_active_asp_artifact_receipt(
    binary_path: &Path,
    binary_digest: &str,
    activation_path: &Path,
) -> Result<ActiveAspArtifactMaterialization, String> {
    let binary_path = canonical_regular_file(binary_path, "ASP binary")?;
    let activation_path = canonical_regular_file(activation_path, "activation")?;
    let receipt_path = active_asp_artifact_receipt_path(&activation_path)?;
    let previous_receipt_bytes = if receipt_path.is_file() {
        let bytes = fs::read(&receipt_path).map_err(|error| {
            format!(
                "failed to read active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
        let receipt: ActiveAspArtifactReceipt =
            serde_json::from_slice(&bytes).map_err(|error| {
                format!(
                    "failed to decode active ASP artifact receipt {}: {error}",
                    receipt_path.display()
                )
            })?;
        receipt.validate().map_err(|error| {
            format!(
                "invalid active ASP artifact receipt {}: {error:?}",
                receipt_path.display()
            )
        })?;
        Some((bytes, receipt))
    } else {
        None
    };
    let binary_metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("failed to inspect {}: {error}", binary_path.display()))?;
    let activation_bytes = fs::read(&activation_path)
        .map_err(|error| format!("failed to read {}: {error}", activation_path.display()))?;
    let binary_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(binary_digest)
            .map_err(|_| format!("invalid BLAKE3 ASP binary digest: {binary_digest}"))?
            .into_content_digest();
    let artifact_byte_reads = 0;
    let artifact_bytes_read = 0;
    let leaves = vec![
        ActiveArtifactLeaf::new(
            ActiveArtifactLeafInput::new(
                "runtime/asp",
                utf8_path(&binary_path, "ASP binary")?,
                ActiveArtifactKind::AspBinary,
                binary_digest,
            )
            .with_materialization_metadata(
                binary_metadata.len(),
                modified_unix_nanos(&binary_metadata)?,
                change_time_unix_nanos(&binary_metadata),
            ),
        )?,
        ActiveArtifactLeaf::new(
            ActiveArtifactLeafInput::new(
                "state/activation.json",
                utf8_path(&activation_path, "activation")?,
                ActiveArtifactKind::Activation,
                blake3_content_digest_v1(&activation_bytes),
            )
            .with_materialization_metadata(
                activation_bytes.len() as u64,
                modified_unix_nanos(&fs::metadata(&activation_path).map_err(|error| {
                    format!("failed to inspect {}: {error}", activation_path.display())
                })?)?,
                change_time_unix_nanos(&fs::metadata(&activation_path).map_err(|error| {
                    format!("failed to inspect {}: {error}", activation_path.display())
                })?),
            ),
        )?,
    ];
    let receipt = ActiveAspArtifactReceipt::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    if previous_receipt_bytes
        .as_ref()
        .is_some_and(|(_, previous)| previous == &receipt)
    {
        return Ok(ActiveAspArtifactMaterialization {
            receipt_path,
            receipt,
            artifact_byte_reads,
            artifact_bytes_read,
            receipt_writes: 0,
        });
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write_compare_exchange(
        &receipt_path,
        &bytes,
        previous_receipt_bytes
            .as_ref()
            .map(|(bytes, _)| bytes.as_slice()),
    )?;
    Ok(ActiveAspArtifactMaterialization {
        receipt_path,
        receipt,
        artifact_byte_reads,
        artifact_bytes_read,
        receipt_writes: 1,
    })
}

#[path = "active_artifact_receipt_verification.rs"]
mod verification;
pub use verification::verify_active_asp_artifact_receipt;
use verification::{change_time_unix_nanos, modified_unix_nanos};

fn canonical_regular_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!(
            "{label} is not a regular file: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn utf8_path(path: &Path, label: &str) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{label} path is not UTF-8: {}", path.display()))
}

fn atomic_write_compare_exchange(
    path: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "active artifact receipt path has no parent: {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let lock_path = parent.join(format!(".{ACTIVE_ASP_ARTIFACT_RECEIPT_FILE}.lock"));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("failed to open {}: {error}", lock_path.display()))?;
    lock.lock_exclusive()
        .map_err(|error| format!("failed to lock {}: {error}", lock_path.display()))?;
    let current = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    if current.as_deref() != expected {
        return Err(format!(
            "active artifact receipt compare-and-swap conflict: {}",
            path.display()
        ));
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(
        ".{ACTIVE_ASP_ARTIFACT_RECEIPT_FILE}.{}.{nonce}.tmp",
        process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("failed to publish {}: {error}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_receipt.rs"]
mod active_artifact_receipt_tests;
