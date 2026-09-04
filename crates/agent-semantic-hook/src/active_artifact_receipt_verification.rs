use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeaf;
use agent_semantic_content_identity::active_artifact_merkle::ActiveAspArtifactReceipt;
use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;

use super::ActiveArtifactMetadataFingerprint;
use super::VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE;
use super::VerifiedActiveAspArtifactReceiptCacheEntry;
use super::active_asp_artifact_receipt_path;
use super::canonical_regular_file;
use super::utf8_path;

pub fn verify_active_asp_artifact_receipt(
    activation_path: &Path,
    asp_paths: &[&Path],
) -> Result<ActiveAspArtifactReceipt, String> {
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    let receipt_metadata = fs::metadata(&receipt_path)
        .map_err(|error| format!("failed to inspect {}: {error}", receipt_path.display()))?;
    if let Some(receipt) = verified_active_receipt_cache_hit(
        &receipt_path,
        &receipt_metadata,
        activation_path,
        asp_paths,
    )? {
        return Ok(receipt);
    }

    let bytes = fs::read(&receipt_path)
        .map_err(|error| format!("failed to read {}: {error}", receipt_path.display()))?;
    let receipt: ActiveAspArtifactReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", receipt_path.display()))?;
    receipt
        .validate()
        .map_err(|error| format!("invalid active ASP artifact receipt: {error:?}"))?;

    let mut leaf_fingerprints = Vec::with_capacity(receipt.leaves().len());
    leaf_fingerprints.push(verify_materialized_leaf(
        activation_path,
        receipt.activation_leaf(),
        "activation",
        MaterializationMatchPolicy::Exact,
    )?);
    for asp_path in asp_paths {
        leaf_fingerprints.push(verify_materialized_leaf(
            asp_path,
            receipt.asp_binary_leaf(),
            "ASP binary",
            MaterializationMatchPolicy::ContentEquivalentAlias,
        )?);
    }
    remember_verified_active_receipt(
        receipt_path,
        &receipt_metadata,
        activation_path,
        asp_paths,
        leaf_fingerprints,
        receipt.clone(),
    )?;
    Ok(receipt)
}

fn verified_active_receipt_cache_hit(
    receipt_path: &Path,
    receipt_metadata: &fs::Metadata,
    activation_path: &Path,
    asp_paths: &[&Path],
) -> Result<Option<ActiveAspArtifactReceipt>, String> {
    let Some(cache) = VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE.get() else {
        return Ok(None);
    };
    let guard = cache
        .lock()
        .map_err(|_| "active ASP artifact receipt cache lock poisoned".to_string())?;
    let Some(entry) = guard.as_ref() else {
        return Ok(None);
    };
    if entry.receipt_path != receipt_path
        || entry.receipt_size_bytes != receipt_metadata.len()
        || entry.receipt_modified_unix_nanos != modified_unix_nanos(receipt_metadata)?
        || entry.receipt_change_time_unix_nanos != change_time_unix_nanos(receipt_metadata)
        || entry.activation_path != activation_path
        || !same_asp_paths(&entry.asp_paths, asp_paths)
    {
        return Ok(None);
    }
    for fingerprint in &entry.leaf_fingerprints {
        if !current_metadata_matches_fingerprint(fingerprint)? {
            return Ok(None);
        }
    }
    Ok(Some(entry.receipt.clone()))
}

fn remember_verified_active_receipt(
    receipt_path: PathBuf,
    receipt_metadata: &fs::Metadata,
    activation_path: &Path,
    asp_paths: &[&Path],
    leaf_fingerprints: Vec<ActiveArtifactMetadataFingerprint>,
    receipt: ActiveAspArtifactReceipt,
) -> Result<(), String> {
    let cache = VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| "active ASP artifact receipt cache lock poisoned".to_string())?;
    *guard = Some(VerifiedActiveAspArtifactReceiptCacheEntry {
        receipt_path,
        receipt_size_bytes: receipt_metadata.len(),
        receipt_modified_unix_nanos: modified_unix_nanos(receipt_metadata)?,
        receipt_change_time_unix_nanos: change_time_unix_nanos(receipt_metadata),
        activation_path: activation_path.to_path_buf(),
        asp_paths: asp_paths.iter().map(|path| (*path).to_path_buf()).collect(),
        leaf_fingerprints,
        receipt,
    });
    Ok(())
}

fn same_asp_paths(cached: &[PathBuf], current: &[&Path]) -> bool {
    cached.len() == current.len()
        && cached
            .iter()
            .zip(current.iter())
            .all(|(cached, current)| cached == *current)
}

fn current_metadata_matches_fingerprint(
    fingerprint: &ActiveArtifactMetadataFingerprint,
) -> Result<bool, String> {
    let metadata = fs::metadata(&fingerprint.materialized_path).map_err(|error| {
        format!(
            "failed to inspect {}: {error}",
            fingerprint.materialized_path
        )
    })?;
    Ok(metadata.len() == fingerprint.size_bytes
        && modified_unix_nanos(&metadata)? == fingerprint.modified_unix_nanos
        && change_time_unix_nanos(&metadata) == fingerprint.change_time_unix_nanos)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MaterializationMatchPolicy {
    Exact,
    ContentEquivalentAlias,
}

pub(super) fn verify_materialized_leaf(
    path: &Path,
    leaf: &ActiveArtifactLeaf,
    label: &str,
    match_policy: MaterializationMatchPolicy,
) -> Result<ActiveArtifactMetadataFingerprint, String> {
    let canonical = canonical_regular_file(path, label)?;
    let is_receipt_materialization = canonical == Path::new(leaf.materialized_path());
    if !is_receipt_materialization && match_policy == MaterializationMatchPolicy::Exact {
        return Err(format!(
            "{label} target mismatch: actual={} receipt={}",
            canonical.display(),
            leaf.materialized_path()
        ));
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect {}: {error}", canonical.display()))?;
    let size = metadata.len();
    if size != leaf.size_bytes() {
        return Err(format!(
            "{label} size mismatch: actual={size} receipt={}",
            leaf.size_bytes()
        ));
    }
    let modified_unix_nanos = modified_unix_nanos(&metadata)?;
    let change_time_unix_nanos = change_time_unix_nanos(&metadata);
    let fingerprint = ActiveArtifactMetadataFingerprint {
        materialized_path: utf8_path(&canonical, label)?,
        size_bytes: size,
        modified_unix_nanos,
        change_time_unix_nanos,
    };
    if is_receipt_materialization
        && modified_unix_nanos == leaf.modified_unix_nanos()
        && change_time_unix_nanos == leaf.change_time_unix_nanos()
    {
        return Ok(fingerprint);
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read {}: {error}", canonical.display()))?;
    let digest = blake3_content_digest_v1(&bytes);
    if &digest != leaf.artifact_digest() {
        return Err(format!(
            "{label} content identity mismatch: actual={} receipt={}",
            digest.as_str(),
            leaf.artifact_digest().as_str()
        ));
    }
    Ok(fingerprint)
}

pub(super) fn modified_unix_nanos(metadata: &fs::Metadata) -> Result<u64, String> {
    let nanos = metadata
        .modified()
        .map_err(|error| format!("failed to read artifact modification time: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("artifact modification time predates UNIX epoch: {error}"))?
        .as_nanos();
    u64::try_from(nanos).map_err(|_| "artifact modification time exceeds u64".to_string())
}

#[cfg(unix)]
pub(super) fn change_time_unix_nanos(metadata: &fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    metadata
        .ctime()
        .checked_mul(1_000_000_000)?
        .checked_add(metadata.ctime_nsec())
}

#[cfg(not(unix))]
pub(super) fn change_time_unix_nanos(_metadata: &fs::Metadata) -> Option<i64> {
    None
}
