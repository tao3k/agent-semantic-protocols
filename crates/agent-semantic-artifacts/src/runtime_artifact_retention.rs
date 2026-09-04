//! Reachability retention and mutation locking owned by the Artifacts package.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-retention-receipt";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactRetentionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub content_algorithm: String,
    pub retention_policy: String,
    pub retained_slots_per_binary: usize,
    pub scanned_generation_count: usize,
    pub retained_generation_count: usize,
    pub removed_generation_count: usize,
    pub ignored_entry_count: usize,
    pub reclaimed_bytes: u64,
    pub protected_digests: Vec<String>,
}

#[derive(Debug)]
struct ArtifactGeneration {
    bytes: u64,
}

pub struct RuntimeArtifactMutationGuard {
    _file: std::sync::Arc<std::fs::File>,
    artifact_root: PathBuf,
}

impl Drop for RuntimeArtifactMutationGuard {
    fn drop(&mut self) {
        if std::sync::Arc::strong_count(&self._file) == 1 {
            let _ = fs2::FileExt::unlock(self._file.as_ref());
        }
    }
}

impl RuntimeArtifactMutationGuard {
    pub fn try_acquire(artifact_root: &Path) -> Result<Self, String> {
        acquire_runtime_artifact_mutation_guard(artifact_root)
    }

    pub(crate) fn admits(&self, artifact_root: &Path) -> bool {
        self.artifact_root == artifact_root
    }
}

fn acquire_runtime_artifact_mutation_guard(
    artifact_root: &Path,
) -> Result<RuntimeArtifactMutationGuard, String> {
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "Runtime artifact root has no Runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let lock_dir = runtime_root.join("locks");
    std::fs::create_dir_all(&lock_dir)
        .map_err(|error| format!("failed to create {}: {error}", lock_dir.display()))?;
    let lock_path = lock_dir.join("artifact-mutation.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open Runtime artifact mutation lock {}: {error}",
                lock_path.display()
            )
        })?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
        format!(
            "Runtime artifact mutation is already active: lock={} reasonKind=artifact-publication-conflict error={error}",
            lock_path.display()
        )
    })?;
    Ok(RuntimeArtifactMutationGuard {
        _file: std::sync::Arc::new(file),
        artifact_root: artifact_root.to_path_buf(),
    })
}

/// Removes every digest generation that is not reachable from a stable Runtime slot.
///
/// Runtime owns both publication roots. Protocol/provider clients must not add their
/// own retention policy or retain rollback generations outside this reachability set.
pub async fn prune_unreachable_runtime_artifacts(
    artifact_root: &Path,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    let artifact_root = artifact_root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _mutation_guard = acquire_runtime_artifact_mutation_guard(&artifact_root)?;
        prune_unreachable_runtime_artifacts_blocking(&artifact_root)
    })
    .await
    .map_err(|error| format!("runtime artifact retention task failed: {error}"))?
}

pub async fn prune_unreachable_runtime_artifacts_under_guard(
    artifact_root: &Path,
    guard: &RuntimeArtifactMutationGuard,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    if !guard.admits(artifact_root) {
        return Err(format!(
            "Runtime artifact mutation guard does not admit {}",
            artifact_root.display()
        ));
    }
    let artifact_root = artifact_root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        prune_unreachable_runtime_artifacts_blocking(&artifact_root)
    })
    .await
    .map_err(|error| format!("Runtime artifact retention task failed: {error}"))?
}

pub(crate) fn prune_unreachable_runtime_artifacts_blocking(
    artifact_root: &Path,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    let algorithm_root = artifact_root.join("blake3-256");
    let (generations, ignored_entry_count) = runtime_artifact_generations(&algorithm_root)?;
    let canonical_algorithm_root =
        std::fs::canonicalize(&algorithm_root).unwrap_or_else(|_| algorithm_root.clone());
    let protected = protected_runtime_artifact_digests(artifact_root, &canonical_algorithm_root)?;
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
        content_algorithm: "blake3-256".to_owned(),
        retention_policy: "active-healthy-reachability".to_owned(),
        retained_slots_per_binary: 2,
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

fn protected_runtime_artifact_digests(
    artifact_root: &Path,
    algorithm_root: &Path,
) -> Result<BTreeSet<String>, String> {
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let mut protected = BTreeSet::new();
    for stable_root in [
        runtime_root.join("bin"),
        runtime_root.join("profiles"),
        runtime_root.join("resident/active"),
        runtime_root.join("resident/healthy"),
    ] {
        collect_reachable_digests(&stable_root, algorithm_root, &mut protected)?;
    }
    Ok(protected)
}

fn collect_reachable_digests(
    path: &Path,
    algorithm_root: &Path,
    protected: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !path.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read entry in {}: {error}", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry_path.display()))?;
        if file_type.is_dir() && !file_type.is_symlink() {
            collect_reachable_digests(&entry_path, algorithm_root, protected)?;
            continue;
        }
        let Ok(identity) = fs::canonicalize(&entry_path) else {
            continue;
        };
        let Ok(relative) = identity.strip_prefix(algorithm_root) else {
            continue;
        };
        let Some(digest) = relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str())
        else {
            continue;
        };
        if valid_digest(digest) {
            protected.insert(digest.to_owned());
        }
    }
    Ok(())
}

fn publish_retention_receipt(
    artifact_root: &Path,
    receipt: &RuntimeArtifactRetentionReceipt,
) -> Result<(), String> {
    fs::create_dir_all(artifact_root)
        .map_err(|error| format!("failed to create {}: {error}", artifact_root.display()))?;
    let path = artifact_root.join("retention-receipt.json");
    let staged = artifact_root.join(format!(
        ".retention-receipt.json.tmp-{}",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("failed to encode runtime artifact retention receipt: {error}"))?;
    fs::write(&staged, bytes)
        .map_err(|error| format!("failed to write {}: {error}", staged.display()))?;
    fs::rename(&staged, &path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
