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
            .join("candidates")
            .join(binary_name);
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
        fs2::FileExt::try_lock_shared(&file).map_err(|error| {
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

struct RuntimeArtifactCandidateRetirementGuard {
    file: std::fs::File,
    lease_path: PathBuf,
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
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "runtime artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    // Retire completed artifact bundles before deriving CAS reachability.
    // Candidates which have not staged activation.json may belong to a
    // concurrent publisher that materialized outside the mutation guard.
    let transaction = format!(
        "{}-{}",
        std::process::id(),
        RETENTION_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
    );
    let retired_artifact_root = artifact_root.join("retired").join(&transaction);
    let retired_candidate_root = runtime_root.join("resident/retired").join(&transaction);
    let (scanned_candidate_count, removed_candidate_count, candidate_ignored) =
        retire_unreachable_runtime_candidates(runtime_root, &retired_candidate_root)?;
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
    }

    let ignored_entry_count = ignored_entry_count.saturating_add(candidate_ignored);

    let receipt = RuntimeArtifactRetentionReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        content_algorithm: "blake3-256".to_owned(),
        retention_policy: "active-healthy-reachability".to_owned(),
        retained_slots_per_binary: 2,
        scanned_generation_count: generations.len(),
        retained_generation_count: generations.len() - removed_generation_count,
        removed_generation_count,
        scanned_candidate_count,
        retained_candidate_count: scanned_candidate_count - removed_candidate_count,
        removed_candidate_count,
        ignored_entry_count,
        reclaimed_bytes,
        protected_digests: protected.into_iter().collect(),
    };
    Ok(PreparedRuntimeArtifactRetention {
        artifact_root: artifact_root.to_path_buf(),
        retired_roots: vec![retired_artifact_root, retired_candidate_root],
        receipt,
    })
}

fn retire_unreachable_runtime_candidates(
    runtime_root: &Path,
    retired_root: &Path,
) -> Result<(usize, usize, usize), String> {
    let artifact_root = runtime_root.join("artifacts");
    let slots_root = artifact_root.to_path_buf();
    let candidate_root = artifact_root.join("bundles");
    if !candidate_root.is_dir() {
        return Ok((0, 0, 0));
    }

    let mut protected = BTreeSet::new();
    for slot in [slots_root.join("active"), slots_root.join("healthy")] {
        match fs::canonicalize(&slot) {
            Ok(target) => {
                protected.insert(target);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to resolve Runtime bundle slot {}: {error}",
                    slot.display()
                ));
            }
        }
    }

    let mut scanned = 0_usize;
    let mut removed = 0_usize;
    let mut ignored = 0_usize;
    for binary_entry in fs::read_dir(&candidate_root)
        .map_err(|error| format!("failed to read {}: {error}", candidate_root.display()))?
    {
        let binary_entry = binary_entry.map_err(|error| {
            format!(
                "failed to read entry in {}: {error}",
                candidate_root.display()
            )
        })?;
        let binary_type = binary_entry.file_type().map_err(|error| {
            format!(
                "failed to inspect {}: {error}",
                binary_entry.path().display()
            )
        })?;
        if !binary_type.is_dir() || binary_type.is_symlink() {
            ignored += 1;
            continue;
        }
        let binary_root = binary_entry.path();
        for candidate_entry in fs::read_dir(&binary_root)
            .map_err(|error| format!("failed to read {}: {error}", binary_root.display()))?
        {
            let candidate_entry = candidate_entry.map_err(|error| {
                format!("failed to read entry in {}: {error}", binary_root.display())
            })?;
            let candidate_type = candidate_entry.file_type().map_err(|error| {
                format!(
                    "failed to inspect {}: {error}",
                    candidate_entry.path().display()
                )
            })?;
            let name = candidate_entry.file_name();
            let is_digest_directory = candidate_type.is_dir()
                && !candidate_type.is_symlink()
                && name.to_str().is_some_and(valid_digest);
            if !is_digest_directory {
                ignored += 1;
                continue;
            }
            scanned += 1;
            let candidate = fs::canonicalize(candidate_entry.path()).map_err(|error| {
                format!(
                    "failed to resolve Runtime artifact bundle {}: {error}",
                    candidate_entry.path().display()
                )
            })?;
            if protected.contains(&candidate) {
                continue;
            }
            let Some(retirement_guard) = acquire_candidate_retirement_guard(
                runtime_root,
                binary_entry.file_name().as_os_str(),
                name.as_os_str(),
            )?
            else {
                continue;
            };
            let binary_name = binary_entry.file_name();
            let retired_binary_root = retired_root.join(binary_name);
            fs::create_dir_all(&retired_binary_root).map_err(|error| {
                format!(
                    "failed to create Runtime candidate retirement root {}: {error}",
                    retired_binary_root.display()
                )
            })?;
            let retired = retired_binary_root.join(&name);
            fs::rename(&candidate, &retired).map_err(|error| {
                format!(
                    "failed to retire unreachable Runtime artifact bundle {} to {}: {error}",
                    candidate.display(),
                    retired.display()
                )
            })?;
            let retired_lease =
                retired_binary_root.join(format!(".{}.preparation.lock", name.to_string_lossy()));
            fs::rename(&retirement_guard.lease_path, &retired_lease).map_err(|error| {
                format!(
                    "failed to retire Runtime candidate preparation lease {} to {}: {error}",
                    retirement_guard.lease_path.display(),
                    retired_lease.display()
                )
            })?;
            fs2::FileExt::unlock(&retirement_guard.file).map_err(|error| {
                format!(
                    "failed to release Runtime candidate retirement lease for {}: {error}",
                    candidate.display()
                )
            })?;
            removed += 1;
        }
    }
    Ok((scanned, removed, ignored))
}

fn acquire_candidate_retirement_guard(
    runtime_root: &Path,
    binary_name: &std::ffi::OsStr,
    bundle_digest: &std::ffi::OsStr,
) -> Result<Option<RuntimeArtifactCandidateRetirementGuard>, String> {
    let lease_root = runtime_root
        .join("artifacts/leases/candidates")
        .join(binary_name);
    fs::create_dir_all(&lease_root).map_err(|error| {
        format!(
            "failed to create Runtime candidate lease root {}: {error}",
            lease_root.display()
        )
    })?;
    let lease_path = lease_root.join(bundle_digest).with_extension("lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lease_path)
        .map_err(|error| {
            format!(
                "failed to open Runtime candidate retirement lease {}: {error}",
                lease_path.display()
            )
        })?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(RuntimeArtifactCandidateRetirementGuard {
            file,
            lease_path,
        })),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(format!(
            "failed to acquire Runtime candidate retirement lease {}: {error}",
            lease_path.display()
        )),
    }
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
        artifact_root.join("active"),
        artifact_root.join("healthy"),
        artifact_root.join("bundles"),
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
