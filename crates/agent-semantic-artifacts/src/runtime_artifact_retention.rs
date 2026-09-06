// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Reachability retention and mutation locking owned by the Artifacts package.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use serde::Serialize;

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-artifact-retention-receipt";
static RETENTION_TRANSACTION_ID: AtomicU64 = AtomicU64::new(1);

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
    #[serde(default)]
    pub scanned_candidate_count: usize,
    #[serde(default)]
    pub retained_candidate_count: usize,
    #[serde(default)]
    pub removed_candidate_count: usize,
    #[serde(default)]
    pub retired_superseded_store_count: usize,
    #[serde(default)]
    pub retired_superseded_lease_directory_count: usize,
    #[serde(default)]
    pub retired_misplaced_launcher_root_count: usize,
    #[serde(default)]
    pub retired_superseded_provider_staging_root_count: usize,
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

pub(crate) struct RuntimeArtifactCandidatePreparationLease {
    file: std::fs::File,
}

impl RuntimeArtifactCandidatePreparationLease {
    pub(crate) fn acquire(
        state_home: &Path,
        binary_name: &str,
        bundle_digest: &str,
    ) -> Result<Self, String> {
        let binary = Path::new(binary_name);
        if binary.components().count() != 1
            || binary_name == "."
            || binary_name == ".."
            || !valid_digest(bundle_digest)
        {
            return Err("invalid Runtime candidate preparation lease identity".to_owned());
        }
        let lease_root = crate::RuntimeArtifactStateLayout::new(state_home)
            .leases()
            .join("candidates");
        fs::create_dir_all(&lease_root).map_err(|error| {
            format!(
                "failed to create Runtime candidate lease root {}: {error}",
                lease_root.display()
            )
        })?;
        let lease_path = lease_root.join(format!("{bundle_digest}.lock"));
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lease_path)
            .map_err(|error| {
                format!(
                    "failed to open Runtime candidate preparation lease {}: {error}",
                    lease_path.display()
                )
            })?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
            format!(
                "state=runtime-artifact-publication-failed reasonKind=candidate-preparation-conflict lease={} error={error}",
                lease_path.display()
            )
        })?;
        Ok(Self { file })
    }
}

impl Drop for RuntimeArtifactCandidatePreparationLease {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

pub(crate) struct PreparedRuntimeArtifactRetention {
    artifact_root: PathBuf,
    retired_roots: Vec<PathBuf>,
    receipt: RuntimeArtifactRetentionReceipt,
}

impl PreparedRuntimeArtifactRetention {
    pub(crate) async fn finish(self) -> Result<RuntimeArtifactRetentionReceipt, String> {
        tokio::task::spawn_blocking(move || finish_prepared_runtime_artifact_retention(self))
            .await
            .map_err(|error| format!("Runtime artifact retention task failed: {error}"))?
    }
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
    artifact_root.parent().ok_or_else(|| {
        format!(
            "Runtime artifact root has no Runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let lock_dir = artifact_root.join("leases");
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
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)?;
    let prepared = prepare_runtime_artifact_retention_under_guard(&artifact_root, &guard).await?;
    drop(guard);
    prepared.finish().await
}

pub async fn prune_unreachable_runtime_artifacts_under_guard(
    artifact_root: &Path,
    guard: &RuntimeArtifactMutationGuard,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    prepare_runtime_artifact_retention_under_guard(artifact_root, guard)
        .await?
        .finish()
        .await
}

pub(crate) async fn prepare_runtime_artifact_retention_under_guard(
    artifact_root: &Path,
    guard: &RuntimeArtifactMutationGuard,
) -> Result<PreparedRuntimeArtifactRetention, String> {
    if !guard.admits(artifact_root) {
        return Err(format!(
            "Runtime artifact mutation guard does not admit {}",
            artifact_root.display()
        ));
    }
    // This phase performs only bounded directory enumeration, metadata reads,
    // and same-filesystem renames. Scheduling it onto the blocking pool while
    // holding the mutation guard makes scheduler latency part of the critical
    // section; recursive deletion remains in `finish` after the guard drops.
    prepare_runtime_artifact_retention(artifact_root)
}

pub(crate) fn prune_unreachable_runtime_artifacts_blocking(
    artifact_root: &Path,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    finish_prepared_runtime_artifact_retention(prepare_runtime_artifact_retention(artifact_root)?)
}

fn prepare_runtime_artifact_retention(
    artifact_root: &Path,
) -> Result<PreparedRuntimeArtifactRetention, String> {
    let layout = crate::RuntimeArtifactStateLayout::from_artifact_root(artifact_root);
    let transaction = format!(
        "{}-{}",
        std::process::id(),
        RETENTION_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
    );
    let retired_artifact_root = artifact_root.join("retired").join(&transaction);
    let generation_root = layout.generation_store();
    let (generations, ignored_entry_count) = runtime_artifact_generations(&generation_root)?;
    let canonical_generation_root =
        std::fs::canonicalize(&generation_root).unwrap_or_else(|_| generation_root.clone());
    let protected = protected_runtime_generation_digests(&layout, &canonical_generation_root)?;
    let mut reclaimed_bytes = 0_u64;
    let mut removed_generation_count = 0_usize;

    for (digest, generation) in &generations {
        if protected.contains(digest) {
            continue;
        }
        if generation_has_live_preparation_lease(artifact_root, digest)? {
            continue;
        }
        let path = generation_root.join(digest);
        fs::create_dir_all(&retired_artifact_root).map_err(|error| {
            format!(
                "failed to create Runtime artifact retirement root {}: {error}",
                retired_artifact_root.display()
            )
        })?;
        let retired = retired_artifact_root.join(digest);
        fs::rename(&path, &retired).map_err(|error| {
            format!(
                "failed to retire unreachable Runtime artifact {} to {}: {error}",
                path.display(),
                retired.display()
            )
        })?;
        reclaimed_bytes = reclaimed_bytes.saturating_add(generation.bytes);
        removed_generation_count += 1;
        remove_dead_generation_preparation_leases(artifact_root, digest)?;
    }

    // The old member-CAS and binary-namespaced bundle trees were two
    // additional physical authorities for the same executable content.  Once
    // at least one canonical generation is selected by active/healthy, retire
    // those exact roots under the same artifact mutation transaction.  Never
    // infer arbitrary siblings and never follow a symlink at either name.
    let retired_superseded_store_count = if protected.is_empty() {
        0
    } else {
        retire_superseded_physical_stores(artifact_root, &retired_artifact_root)?
    };
    let retired_superseded_lease_directory_count = if protected.is_empty() {
        0
    } else {
        retire_superseded_candidate_lease_directories(artifact_root, &retired_artifact_root)?
    };
    let retired_misplaced_launcher_root_count = if protected.is_empty() {
        0
    } else {
        retire_misplaced_state_home_launcher_root(artifact_root, &retired_artifact_root)?
    };
    let retired_superseded_provider_staging_root_count = if protected.is_empty() {
        0
    } else {
        retire_superseded_provider_staging_root(artifact_root, &retired_artifact_root)?
    };

    let receipt = RuntimeArtifactRetentionReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        content_algorithm: "blake3-256".to_owned(),
        retention_policy: "active-healthy-reachability".to_owned(),
        retained_slots_per_binary: 2,
        scanned_generation_count: generations.len(),
        retained_generation_count: generations.len() - removed_generation_count,
        removed_generation_count,
        scanned_candidate_count: 0,
        retained_candidate_count: 0,
        removed_candidate_count: 0,
        retired_superseded_store_count,
        retired_superseded_lease_directory_count,
        retired_misplaced_launcher_root_count,
        retired_superseded_provider_staging_root_count,
        ignored_entry_count,
        reclaimed_bytes,
        protected_digests: protected.into_iter().collect(),
    };
    Ok(PreparedRuntimeArtifactRetention {
        artifact_root: artifact_root.to_path_buf(),
        retired_roots: vec![retired_artifact_root],
        receipt,
    })
}

fn retire_superseded_provider_staging_root(
    artifact_root: &Path,
    retired_artifact_root: &Path,
) -> Result<usize, String> {
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "Runtime artifact root has no Runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let source = runtime_root.join("providers");
    let metadata = match fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "failed to inspect superseded Provider staging root {}: {error}",
                source.display()
            ));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "state=runtime-artifact-retention-failed reasonKind=superseded-provider-staging-root-type-conflict path={}",
            source.display()
        ));
    }
    let retirement_root = retired_artifact_root.join("superseded-provider-staging");
    fs::create_dir_all(&retirement_root).map_err(|error| {
        format!(
            "failed to create superseded Provider staging retirement root {}: {error}",
            retirement_root.display()
        )
    })?;
    let target = retirement_root.join("providers");
    fs::rename(&source, &target).map_err(|error| {
        format!(
            "failed to retire superseded Provider staging root {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    Ok(1)
}

fn retire_misplaced_state_home_launcher_root(
    artifact_root: &Path,
    retired_artifact_root: &Path,
) -> Result<usize, String> {
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "Runtime artifact root has no Runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let state_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "Runtime root has no State Home parent: {}",
            runtime_root.display()
        )
    })?;
    let misplaced_root = state_home.join("bin");
    let metadata = match fs::symlink_metadata(&misplaced_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "failed to inspect misplaced State Home launcher root {}: {error}",
                misplaced_root.display()
            ));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "state=runtime-artifact-retention-failed reasonKind=misplaced-launcher-root-type-conflict path={}",
            misplaced_root.display()
        ));
    }
    let active_generation = fs::canonicalize(artifact_root.join("active"))
        .map_err(|error| format!("failed to resolve active Runtime generation: {error}"))?;
    for entry in fs::read_dir(&misplaced_root)
        .map_err(|error| format!("failed to read {}: {error}", misplaced_root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read entry in misplaced launcher root {}: {error}",
                misplaced_root.display()
            )
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if !file_type.is_symlink() {
            return Err(format!(
                "state=runtime-artifact-retention-failed reasonKind=misplaced-launcher-entry-not-managed path={}",
                entry.path().display()
            ));
        }
        let target = fs::canonicalize(entry.path()).map_err(|error| {
            format!(
                "failed to resolve misplaced launcher {}: {error}",
                entry.path().display()
            )
        })?;
        let relative = target.strip_prefix(&active_generation).map_err(|_| {
            format!(
                "state=runtime-artifact-retention-failed reasonKind=misplaced-launcher-target-drift path={} target={}",
                entry.path().display(),
                target.display()
            )
        })?;
        if relative.components().count() != 1 {
            return Err(format!(
                "state=runtime-artifact-retention-failed reasonKind=misplaced-launcher-target-drift path={} target={}",
                entry.path().display(),
                target.display()
            ));
        }
    }
    let retirement_root = retired_artifact_root.join("misplaced-launchers");
    fs::create_dir_all(&retirement_root).map_err(|error| {
        format!(
            "failed to create misplaced launcher retirement root {}: {error}",
            retirement_root.display()
        )
    })?;
    let target = retirement_root.join("state-home-bin");
    fs::rename(&misplaced_root, &target).map_err(|error| {
        format!(
            "failed to retire misplaced State Home launcher root {} to {}: {error}",
            misplaced_root.display(),
            target.display()
        )
    })?;
    Ok(1)
}

fn retire_superseded_candidate_lease_directories(
    artifact_root: &Path,
    retired_artifact_root: &Path,
) -> Result<usize, String> {
    let lease_root = artifact_root.join("leases/candidates");
    if !lease_root.is_dir() {
        return Ok(0);
    }
    let mut retired = 0_usize;
    for entry in fs::read_dir(&lease_root)
        .map_err(|error| format!("failed to read {}: {error}", lease_root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!("failed to read entry in {}: {error}", lease_root.display())
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "state=runtime-artifact-retention-failed reasonKind=superseded-candidate-lease-type-conflict path={}",
                entry.path().display()
            ));
        }
        if !file_type.is_dir() {
            continue;
        }
        let retirement_root = retired_artifact_root.join("superseded-candidate-leases");
        fs::create_dir_all(&retirement_root).map_err(|error| {
            format!(
                "failed to create superseded candidate lease retirement root {}: {error}",
                retirement_root.display()
            )
        })?;
        let target = retirement_root.join(entry.file_name());
        fs::rename(entry.path(), &target).map_err(|error| {
            format!(
                "failed to retire superseded candidate lease directory {} to {}: {error}",
                entry.path().display(),
                target.display()
            )
        })?;
        retired += 1;
    }
    Ok(retired)
}

fn retire_superseded_physical_stores(
    artifact_root: &Path,
    retired_artifact_root: &Path,
) -> Result<usize, String> {
    let mut retired = 0_usize;
    for name in ["blake3-256", "bundles"] {
        let source = artifact_root.join(name);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "failed to inspect superseded Runtime artifact store {}: {error}",
                    source.display()
                ));
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!(
                "state=runtime-artifact-retention-failed reasonKind=superseded-artifact-store-type-conflict path={}",
                source.display()
            ));
        }
        let retirement_root = retired_artifact_root.join("superseded-stores");
        fs::create_dir_all(&retirement_root).map_err(|error| {
            format!(
                "failed to create superseded Runtime artifact retirement root {}: {error}",
                retirement_root.display()
            )
        })?;
        let target = retirement_root.join(name);
        fs::rename(&source, &target).map_err(|error| {
            format!(
                "failed to retire superseded Runtime artifact store {} to {}: {error}",
                source.display(),
                target.display()
            )
        })?;
        retired += 1;
    }
    Ok(retired)
}

fn generation_has_live_preparation_lease(
    artifact_root: &Path,
    digest: &str,
) -> Result<bool, String> {
    for lease in generation_preparation_leases(artifact_root, digest)? {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease)
            .map_err(|error| {
                format!(
                    "failed to open candidate lease {}: {error}",
                    lease.display()
                )
            })?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => fs2::FileExt::unlock(&file).map_err(|error| {
                format!(
                    "failed to unlock candidate lease {}: {error}",
                    lease.display()
                )
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(true),
            Err(error) => {
                return Err(format!(
                    "failed to inspect candidate lease {}: {error}",
                    lease.display()
                ));
            }
        }
    }
    Ok(false)
}

fn remove_dead_generation_preparation_leases(
    artifact_root: &Path,
    digest: &str,
) -> Result<(), String> {
    for lease in generation_preparation_leases(artifact_root, digest)? {
        match fs::remove_file(&lease) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove candidate lease {}: {error}",
                    lease.display()
                ));
            }
        }
    }
    Ok(())
}

fn generation_preparation_leases(
    artifact_root: &Path,
    digest: &str,
) -> Result<Vec<PathBuf>, String> {
    let root = artifact_root.join("leases/candidates");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let lease = root.join(format!("{digest}.lock"));
    Ok(if lease.is_file() {
        vec![lease]
    } else {
        Vec::new()
    })
}

fn finish_prepared_runtime_artifact_retention(
    prepared: PreparedRuntimeArtifactRetention,
) -> Result<RuntimeArtifactRetentionReceipt, String> {
    for root in &prepared.retired_roots {
        match fs::remove_dir_all(root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove retired Runtime state {}: {error}",
                    root.display()
                ));
            }
        }
    }
    let retired_parent = prepared.artifact_root.join("retired");
    match fs::remove_dir(&retired_parent) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) => {}
        Err(error) => {
            return Err(format!(
                "failed to remove empty Runtime artifact retirement root {}: {error}",
                retired_parent.display()
            ));
        }
    }
    publish_retention_receipt(&prepared.artifact_root, &prepared.receipt)?;
    Ok(prepared.receipt)
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

fn protected_runtime_generation_digests(
    layout: &crate::RuntimeArtifactStateLayout,
    generation_root: &Path,
) -> Result<BTreeSet<String>, String> {
    let mut protected = BTreeSet::new();
    for slot in [layout.active_slot(), layout.healthy_slot()] {
        let target = match fs::canonicalize(&slot) {
            Ok(target) => target,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "failed to resolve Runtime artifact selector {}: {error}",
                    slot.display()
                ));
            }
        };
        let relative = target.strip_prefix(generation_root).map_err(|_| {
            format!(
                "Runtime artifact selector escaped generation store: slot={} target={}",
                slot.display(),
                target.display()
            )
        })?;
        let digest = relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str())
            .filter(|digest| valid_digest(digest))
            .ok_or_else(|| {
                format!(
                    "Runtime artifact selector has invalid generation identity: slot={} target={}",
                    slot.display(),
                    target.display()
                )
            })?;
        protected.insert(digest.to_owned());
    }
    Ok(protected)
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
