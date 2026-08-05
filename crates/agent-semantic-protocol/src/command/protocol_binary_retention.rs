//! Reachability-based retention for immutable runtime binary artifacts.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-retention-receipt";
const ROLLBACK_GENERATIONS_PER_BINARY: usize = 0;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeArtifactRetentionReceipt {
    schema_id: String,
    schema_version: String,
    algorithm: String,
    rollback_generations_per_binary: usize,
    pub(crate) scanned_generation_count: usize,
    pub(crate) retained_generation_count: usize,
    pub(crate) removed_generation_count: usize,
    pub(crate) ignored_entry_count: usize,
    pub(crate) reclaimed_bytes: u64,
    pub(crate) protected_digests: Vec<String>,
}

#[derive(Debug)]
struct ArtifactGeneration {
    bytes: u64,
}

pub(crate) fn prune_runtime_binary_artifacts(
    artifact_root: &Path,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    let algorithm_root = artifact_root.join("blake3-256");
    let (generations, ignored_entry_count) = runtime_artifact_generations(&algorithm_root)?;
    let protected = protected_runtime_artifact_digests(artifact_root)?;
    let mut reclaimed_bytes = 0_u64;
    let mut removed_generation_count = 0_usize;
    for (digest, generation) in &generations {
        if protected.contains(digest) {
            continue;
        }
        let path = algorithm_root.join(digest);
        fs::remove_dir_all(&path)
            .map_err(|error| format!("failed to remove unreachable {}: {error}", path.display()))?;
        reclaimed_bytes = reclaimed_bytes.saturating_add(generation.bytes);
        removed_generation_count += 1;
    }

    let receipt = RuntimeArtifactRetentionReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        algorithm: "blake3-256".to_owned(),
        rollback_generations_per_binary: ROLLBACK_GENERATIONS_PER_BINARY,
        scanned_generation_count: generations.len(),
        retained_generation_count: generations.len() - removed_generation_count,
        removed_generation_count,
        ignored_entry_count,
        reclaimed_bytes,
        protected_digests: protected.into_iter().collect(),
    };
    publish_retention_receipt(artifact_root, &receipt)?;
    Ok(receipt)
}

fn protected_runtime_artifact_digests(artifact_root: &Path) -> Result<BTreeSet<String>, String> {
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let bin_root = runtime_root.join("bin");
    if !bin_root.is_dir() {
        return Ok(BTreeSet::new());
    }
    let mut protected = BTreeSet::new();
    for entry in fs::read_dir(&bin_root)
        .map_err(|error| format!("failed to read {}: {error}", bin_root.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read entry in {}: {error}", bin_root.display()))?;
        let Ok(identity) = fs::canonicalize(entry.path()) else {
            continue;
        };
        if let Some(digest) = super::protocol_binary_digest_from_canonical_artifact_path(&identity)
        {
            protected.insert(digest);
        }
    }
    Ok(protected)
}

fn runtime_artifact_generations(
    algorithm_root: &Path,
) -> Result<(BTreeMap<String, ArtifactGeneration>, usize), String> {
    if !algorithm_root.is_dir() {
        return Ok((BTreeMap::new(), 0));
    }
    let mut generations = BTreeMap::new();
    let mut ignored = 0_usize;
    for entry in fs::read_dir(algorithm_root)
        .map_err(|error| format!("failed to read {}: {error}", algorithm_root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read entry in {}: {error}",
                algorithm_root.display()
            )
        })?;
        let name = entry.file_name();
        if name == "latest" {
            continue;
        }
        let Some(digest) = name.to_str().filter(|value| valid_digest(value)) else {
            ignored += 1;
            continue;
        };
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            ignored += 1;
            continue;
        }
        let (generation, generation_ignored) = artifact_generation(entry.path())?;
        ignored += generation_ignored;
        generations.insert(digest.to_owned(), generation);
    }
    Ok((generations, ignored))
}

fn artifact_generation(path: PathBuf) -> Result<(ArtifactGeneration, usize), String> {
    let mut bytes = 0_u64;
    let mut ignored = 0_usize;
    for entry in fs::read_dir(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read entry in {}: {error}", path.display()))?;
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if !metadata.is_file() || entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            ignored += 1;
            continue;
        }
        bytes = bytes.saturating_add(metadata.len());
    }
    Ok((ArtifactGeneration { bytes }, ignored))
}

fn publish_retention_receipt(
    artifact_root: &Path,
    receipt: &RuntimeArtifactRetentionReceipt,
) -> Result<(), String> {
    fs::create_dir_all(artifact_root)
        .map_err(|error| format!("failed to create {}: {error}", artifact_root.display()))?;
    let path = artifact_root.join("retention-receipt.v1.json");
    let staged = artifact_root.join(format!(
        ".retention-receipt.v1.json.tmp-{}",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("failed to encode runtime artifact retention receipt: {error}"))?;
    fs::write(&staged, bytes)
        .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
    super::atomic_replace_protocol_entry(&staged, &path)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
