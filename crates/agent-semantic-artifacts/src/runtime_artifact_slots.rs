//! Active/healthy bundle slots and immutable candidate materialization.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::runtime_artifact_store::{
    publish_runtime_artifact_link, runtime_artifact_content_digest, stage_runtime_artifact,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactSlotAuthority {
    root: PathBuf,
    artifact_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeArtifactBundleManifest {
    schema_id: String,
    schema_version: u64,
    bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    members: std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
}

pub fn runtime_artifact_bundle_digest(
    members: &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
) -> crate::blake3_content_digest::Blake3ContentDigest {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"agent.semantic-protocols.runtime-binary-bundle\0");
    for (member, digest) in members {
        identity.extend_from_slice(member.as_bytes());
        identity.push(0);
        identity.extend_from_slice(digest.as_str().as_bytes());
        identity.push(0);
    }
    crate::blake3_content_digest::Blake3ContentDigest::from_bytes(&identity)
}

pub async fn runtime_artifact_candidate_bundle_digest(
    candidate: &Path,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    let manifest_path = candidate.join("bundle.json");
    let bytes = tokio::fs::read(&manifest_path).await.map_err(|error| {
        format!(
            "read Runtime artifact bundle manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    let manifest: RuntimeArtifactBundleManifest =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "decode Runtime artifact bundle manifest {}: {error}",
                manifest_path.display()
            )
        })?;
    if manifest.schema_id != "agent.semantic-protocols.runtime-binary-bundle"
        || manifest.schema_version != 1
        || manifest.members.is_empty()
    {
        return Err("invalid Runtime artifact bundle manifest authority".to_owned());
    }
    let derived = runtime_artifact_bundle_digest(&manifest.members);
    if derived != manifest.bundle_digest {
        return Err("Runtime artifact bundle manifest digest mismatch".to_owned());
    }
    for (member, expected_digest) in manifest.members {
        let member_path = Path::new(&member);
        if member_path.components().count() != 1 || member == "." || member == ".." {
            return Err(format!("invalid Runtime artifact bundle member `{member}`"));
        }
        let artifact = candidate.join(&member);
        let observed_digest = runtime_artifact_candidate_digest(&artifact).await?;
        if observed_digest != expected_digest {
            return Err(format!(
                "Runtime artifact bundle member digest mismatch: member={member} expected={expected_digest} observed={observed_digest}"
            ));
        }
    }
    Ok(derived)
}

impl RuntimeArtifactSlotAuthority {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::for_artifact(root, "asp")
    }

    pub fn for_artifact(root: impl Into<PathBuf>, artifact_kind: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            artifact_kind: artifact_kind.into(),
        }
    }

    pub fn active_path(&self) -> PathBuf {
        self.root.join("active")
    }

    pub fn healthy_path(&self) -> PathBuf {
        self.root.join("healthy")
    }

    pub async fn active_target(&self) -> Result<Option<PathBuf>, String> {
        read_runtime_artifact_slot(&self.active_path()).await
    }

    pub(crate) fn active_target_under_guard(&self) -> Result<Option<PathBuf>, String> {
        read_runtime_artifact_slot_under_guard(&self.active_path())
    }

    pub async fn healthy_target(&self) -> Result<Option<PathBuf>, String> {
        read_runtime_artifact_slot(&self.healthy_path()).await
    }

    pub(crate) fn healthy_target_under_guard(&self) -> Result<Option<PathBuf>, String> {
        read_runtime_artifact_slot_under_guard(&self.healthy_path())
    }

    pub async fn commit_ready(&self, candidate: &Path) -> Result<(), String> {
        self.validate_candidate(candidate).await?;
        let previous = self.active_target().await?;
        let healthy = previous.as_deref().unwrap_or(candidate);
        publish_runtime_artifact_slot(healthy, &self.healthy_path()).await?;
        publish_runtime_artifact_slot(candidate, &self.active_path()).await
    }

    /// Atomically expose a complete immutable candidate through the sole active
    /// directory selector. The prior healthy selector is deliberately retained
    /// until Runtime validates the newly active candidate.
    pub async fn publish_active_candidate(&self, candidate: &Path) -> Result<(), String> {
        self.validate_candidate(candidate).await?;
        if self.healthy_target().await?.is_none()
            && let Some(previous) = self.active_target().await?
        {
            publish_runtime_artifact_slot(&previous, &self.healthy_path()).await?;
        }
        publish_runtime_artifact_slot(candidate, &self.active_path()).await
    }

    /// Commit a candidate whose immutable bundle was already validated before
    /// acquiring the artifact mutation guard. This path performs only bounded
    /// symlink reads and atomic renames; it never schedules blocking work.
    pub(crate) fn publish_active_candidate_under_guard(
        &self,
        candidate: &Path,
    ) -> Result<(), String> {
        if self.healthy_target_under_guard()?.is_none()
            && let Some(previous) = self.active_target_under_guard()?
        {
            publish_runtime_artifact_slot_under_guard(&previous, &self.healthy_path())?;
        }
        publish_runtime_artifact_slot_under_guard(candidate, &self.active_path())
    }

    /// Complete validation for the already-active candidate. This never moves
    /// the active selector and therefore cannot select a second Hook/runtime
    /// generation.
    pub async fn mark_active_healthy(&self, candidate: &Path) -> Result<(), String> {
        self.validate_candidate(candidate).await?;
        let active = self.active_target().await?.ok_or_else(|| {
            "Runtime artifact active slot is unavailable during health commit".to_owned()
        })?;
        if active != candidate {
            return Err(format!(
                "Runtime artifact health commit candidate is not active: active={} candidate={}",
                active.display(),
                candidate.display()
            ));
        }
        publish_runtime_artifact_slot(candidate, &self.healthy_path()).await
    }

    /// Roll back only the candidate that is still active. Replaying the same
    /// rollback after the healthy selector has already been restored is an
    /// idempotent no-op. A stale actor may not move either selector after a
    /// newer install has already won the slot.
    pub async fn restore_active_from_healthy(&self, candidate: &Path) -> Result<(), String> {
        let active = self.active_target().await?;
        let healthy = self.healthy_target().await?;
        if active.as_deref() == Some(candidate) {
            return restore_runtime_artifact_slot(healthy.as_deref(), &self.active_path()).await;
        }
        if active.is_some() && active == healthy {
            return Ok(());
        }
        Err(format!(
            "Runtime artifact rollback candidate is neither active nor already restored: active={active:?} healthy={healthy:?} candidate={}",
            candidate.display()
        ))
    }

    pub(crate) async fn validate_candidate(&self, candidate: &Path) -> Result<(), String> {
        let manifest_path = candidate.join("bundle.json");
        match tokio::fs::read(&manifest_path).await {
            Ok(bytes) => {
                let manifest: RuntimeArtifactBundleManifest = serde_json::from_slice(&bytes)
                    .map_err(|error| {
                        format!(
                            "decode Runtime artifact bundle manifest {}: {error}",
                            manifest_path.display()
                        )
                    })?;
                if manifest.schema_id != "agent.semantic-protocols.runtime-binary-bundle"
                    || manifest.schema_version != 1
                    || manifest.members.is_empty()
                {
                    return Err("invalid Runtime artifact bundle manifest authority".to_owned());
                }
                if runtime_artifact_bundle_digest(&manifest.members) != manifest.bundle_digest {
                    return Err("Runtime artifact bundle manifest digest mismatch".to_owned());
                }
                for (member, expected_digest) in manifest.members {
                    let member_path = Path::new(&member);
                    if member_path.components().count() != 1 || member == "." || member == ".." {
                        return Err(format!("invalid Runtime artifact bundle member `{member}`"));
                    }
                    let artifact = candidate.join(&member);
                    let observed_digest = runtime_artifact_candidate_digest(&artifact).await?;
                    if observed_digest != expected_digest {
                        return Err(format!(
                            "Runtime artifact bundle member digest mismatch: member={member} expected={expected_digest} observed={observed_digest}"
                        ));
                    }
                }
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "read Runtime artifact bundle manifest {}: {error}",
                    manifest_path.display()
                ));
            }
        }
        let candidate_artifact = candidate.join(&self.artifact_kind);
        let metadata = tokio::fs::symlink_metadata(&candidate_artifact)
            .await
            .map_err(|error| {
                format!(
                    "candidate publication is missing `{}` at {}: {error}",
                    self.artifact_kind,
                    candidate_artifact.display()
                )
            })?;
        if !metadata.file_type().is_symlink() && !metadata.is_file() {
            return Err(format!(
                "candidate publication artifact is not a file or symlink: {}",
                candidate_artifact.display()
            ));
        }
        Ok(())
    }

    pub(crate) async fn restore_targets(
        &self,
        active: Option<&Path>,
        healthy: Option<&Path>,
    ) -> Result<(), String> {
        restore_runtime_artifact_slot(active, &self.active_path()).await?;
        restore_runtime_artifact_slot(healthy, &self.healthy_path()).await
    }

    pub(crate) fn restore_targets_under_guard(
        &self,
        active: Option<&Path>,
        healthy: Option<&Path>,
    ) -> Result<(), String> {
        restore_runtime_artifact_slot_under_guard(active, &self.active_path())?;
        restore_runtime_artifact_slot_under_guard(healthy, &self.healthy_path())
    }

    pub async fn stage_candidate_artifact(
        &self,
        candidate_dir: &Path,
        artifact: &Path,
    ) -> Result<(), String> {
        self.stage_candidate_member(candidate_dir, &self.artifact_kind, artifact)
            .await
    }

    pub async fn stage_candidate_member(
        &self,
        candidate_dir: &Path,
        member: &str,
        artifact: &Path,
    ) -> Result<(), String> {
        let member_path = Path::new(member);
        if member_path.components().count() != 1 || member == "." || member == ".." {
            return Err(format!("invalid Runtime artifact bundle member `{member}`"));
        }
        publish_runtime_artifact_slot(artifact, &candidate_dir.join(member)).await
    }

    pub async fn prune_unreachable_publications(&self) -> Result<(), String> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let mut protected = std::collections::BTreeSet::new();
            for slot in [root.join("active"), root.join("healthy")] {
                match std::fs::canonicalize(&slot) {
                    Ok(target) => {
                        protected.insert(target);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "resolve Runtime artifact slot {}: {error}",
                            slot.display()
                        ));
                    }
                }
            }
            let publications = root.join("publications");
            let entries = match std::fs::read_dir(&publications) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(format!(
                        "read Runtime artifact publications {}: {error}",
                        publications.display()
                    ));
                }
            };
            for entry in entries {
                let entry = entry
                    .map_err(|error| format!("read Runtime artifact publication entry: {error}"))?;
                let file_type = entry.file_type().map_err(|error| {
                    format!(
                        "read Runtime artifact publication type {}: {error}",
                        entry.path().display()
                    )
                })?;
                if !file_type.is_dir() {
                    continue;
                }
                let path = std::fs::canonicalize(entry.path()).map_err(|error| {
                    format!(
                        "resolve Runtime artifact publication {}: {error}",
                        entry.path().display()
                    )
                })?;
                if !protected.contains(&path) {
                    std::fs::remove_dir_all(&path).map_err(|error| {
                        format!(
                            "remove unreachable Runtime artifact publication {}: {error}",
                            path.display()
                        )
                    })?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("prune Runtime artifact publications task failed: {error}"))?
    }
}

async fn publish_runtime_artifact_slot(target: &Path, slot: &Path) -> Result<(), String> {
    let target = target.to_path_buf();
    let slot = slot.to_path_buf();
    tokio::task::spawn_blocking(move || publish_runtime_artifact_link(&target, &slot))
        .await
        .map_err(|error| format!("publish Runtime artifact slot task failed: {error}"))?
}

fn publish_runtime_artifact_slot_under_guard(target: &Path, slot: &Path) -> Result<(), String> {
    publish_runtime_artifact_link(target, slot)
}

async fn restore_runtime_artifact_slot(target: Option<&Path>, slot: &Path) -> Result<(), String> {
    match target {
        Some(target) => publish_runtime_artifact_slot(target, slot).await,
        None => match tokio::fs::remove_file(slot).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime artifact slot {} during rollback: {error}",
                slot.display()
            )),
        },
    }
}

fn restore_runtime_artifact_slot_under_guard(
    target: Option<&Path>,
    slot: &Path,
) -> Result<(), String> {
    match target {
        Some(target) => publish_runtime_artifact_slot_under_guard(target, slot),
        None => match std::fs::remove_file(slot) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove Runtime artifact slot {} during rollback: {error}",
                slot.display()
            )),
        },
    }
}

async fn read_runtime_artifact_slot(path: &Path) -> Result<Option<PathBuf>, String> {
    match tokio::fs::read_link(path).await {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read Runtime artifact slot {}: {error}",
            path.display()
        )),
    }
}

fn read_runtime_artifact_slot_under_guard(path: &Path) -> Result<Option<PathBuf>, String> {
    match std::fs::read_link(path) {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read Runtime artifact slot {}: {error}",
            path.display()
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRuntimeArtifact {
    pub path: PathBuf,
    pub content_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub was_present: bool,
}

pub async fn publish_resident_runtime_alias(
    target: &Path,
    resident_root: &Path,
) -> Result<(), String> {
    let target = target.to_path_buf();
    let resident_root = resident_root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let parent = target.parent().ok_or_else(|| {
            format!(
                "resident Runtime binary alias has no parent: {}",
                target.display()
            )
        })?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create resident Runtime alias directory: {error}"))?;
        let staged = parent.join(format!(".asp.resident-{}", std::process::id()));
        match std::fs::remove_file(&staged) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove stale resident Runtime alias {}: {error}",
                    staged.display()
                ));
            }
        }
        let artifact_kind = target.file_name().ok_or_else(|| {
            format!(
                "resident Runtime alias has no artifact kind: {}",
                target.display()
            )
        })?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(resident_root.join("active").join(artifact_kind), &staged)
            .map_err(|error| {
                format!("stage resident Runtime alias {}: {error}", staged.display())
            })?;
        #[cfg(not(unix))]
        return Err("resident Runtime publication requires atomic symlink support".to_owned());
        std::fs::rename(&staged, &target).map_err(|error| {
            format!(
                "publish resident Runtime binary alias {}: {error}",
                target.display()
            )
        })
    })
    .await
    .map_err(|error| format!("publish resident Runtime alias task failed: {error}"))?
}

pub async fn runtime_artifact_candidate_digest(
    source: &Path,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    let source = source.to_path_buf();
    tokio::task::spawn_blocking(move || runtime_artifact_content_digest(&source))
        .await
        .map_err(|error| format!("digest Runtime artifact candidate task failed: {error}"))?
}

pub async fn prepare_runtime_artifact_candidate(
    state_home: &Path,
    candidate_dir: &Path,
    source: &Path,
) -> Result<PreparedRuntimeArtifact, String> {
    prepare_runtime_artifact_candidate_for_kind(state_home, candidate_dir, source, "asp").await
}

pub async fn prepare_runtime_artifact_candidate_for_kind(
    state_home: &Path,
    candidate_dir: &Path,
    source: &Path,
    artifact_kind: &str,
) -> Result<PreparedRuntimeArtifact, String> {
    let state_home = state_home.to_path_buf();
    let candidate_dir = candidate_dir.to_path_buf();
    let source = source.to_path_buf();
    let artifact_kind = artifact_kind.to_owned();
    tokio::task::spawn_blocking(move || {
        prepare_runtime_artifact_candidate_blocking(
            &state_home,
            &candidate_dir,
            &source,
            &artifact_kind,
        )
    })
    .await
    .map_err(|error| format!("prepare Runtime artifact candidate task failed: {error}"))?
}

pub async fn prepare_runtime_artifact_bytes_candidate_for_kind(
    state_home: &Path,
    candidate_dir: &Path,
    bytes: &[u8],
    artifact_kind: &str,
) -> Result<PreparedRuntimeArtifact, String> {
    let state_home = state_home.to_path_buf();
    let candidate_dir = candidate_dir.to_path_buf();
    let bytes = bytes.to_vec();
    let artifact_kind = artifact_kind.to_owned();
    tokio::task::spawn_blocking(move || {
        let content_digest = crate::blake3_content_digest::Blake3ContentDigest::from_bytes(&bytes);
        let path = state_home
            .join("runtime/artifacts/blake3-256")
            .join(content_digest.content_digest().as_str())
            .join(&artifact_kind);
        if path.exists() {
            let observed = runtime_artifact_content_digest(&path)?;
            if observed != content_digest {
                return Err(format!(
                    "immutable Runtime artifact digest mismatch: expected={content_digest} observed={observed} path={}",
                    path.display()
                ));
            }
            return Ok(PreparedRuntimeArtifact {
                path,
                content_digest,
                was_present: true,
            });
        }
        let parent = path
            .parent()
            .ok_or_else(|| format!("Runtime artifact path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!("create immutable Runtime artifact directory {}: {error}", parent.display())
        })?;
        std::fs::create_dir_all(&candidate_dir).map_err(|error| {
            format!("create Runtime candidate staging directory {}: {error}", candidate_dir.display())
        })?;
        let staged = candidate_dir.join(format!("{artifact_kind}.immutable"));
        std::fs::write(&staged, &bytes)
            .map_err(|error| format!("stage Runtime artifact bytes {}: {error}", staged.display()))?;
        let staged_digest = runtime_artifact_content_digest(&staged)?;
        if staged_digest != content_digest {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "staged Runtime artifact digest mismatch: expected={content_digest} observed={staged_digest}"
            ));
        }
        let was_present = match std::fs::hard_link(&staged, &path) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
            Err(error) => {
                let _ = std::fs::remove_file(&staged);
                return Err(format!("publish immutable Runtime artifact {}: {error}", path.display()));
            }
        };
        std::fs::remove_file(&staged)
            .map_err(|error| format!("remove staged Runtime artifact {}: {error}", staged.display()))?;
        Ok(PreparedRuntimeArtifact {
            path,
            content_digest,
            was_present,
        })
    })
    .await
    .map_err(|error| format!("prepare Runtime artifact bytes candidate task failed: {error}"))?
}

fn prepare_runtime_artifact_candidate_blocking(
    state_home: &Path,
    candidate_dir: &Path,
    source: &Path,
    artifact_kind: &str,
) -> Result<PreparedRuntimeArtifact, String> {
    let content_digest = runtime_artifact_content_digest(source)?;
    let digest_hex = content_digest.content_digest().as_str();
    let path = state_home
        .join("runtime/artifacts/blake3-256")
        .join(digest_hex)
        .join(artifact_kind);
    if path.exists() {
        let observed = runtime_artifact_content_digest(&path)?;
        if observed != content_digest {
            return Err(format!(
                "immutable Runtime artifact digest mismatch: expected={content_digest} observed={observed} path={}",
                path.display()
            ));
        }
        return Ok(PreparedRuntimeArtifact {
            path,
            content_digest,
            was_present: true,
        });
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("Runtime artifact path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create immutable Runtime artifact directory {}: {error}",
            parent.display()
        )
    })?;
    std::fs::create_dir_all(candidate_dir).map_err(|error| {
        format!(
            "create Runtime candidate staging directory {}: {error}",
            candidate_dir.display()
        )
    })?;
    let staged = candidate_dir.join(format!("{artifact_kind}.immutable"));
    stage_runtime_artifact(source, &staged)?;
    let staged_digest = runtime_artifact_content_digest(&staged)?;
    if staged_digest != content_digest {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "staged Runtime artifact digest mismatch: expected={content_digest} observed={staged_digest}"
        ));
    }
    let was_present = match std::fs::hard_link(&staged, &path) {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(error) => {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "publish immutable Runtime artifact {}: {error}",
                path.display()
            ));
        }
    };
    std::fs::remove_file(&staged).map_err(|error| {
        format!(
            "remove staged Runtime artifact {}: {error}",
            staged.display()
        )
    })?;
    let observed = runtime_artifact_content_digest(&path)?;
    if observed != content_digest {
        if !was_present {
            let _ = std::fs::remove_file(&path);
        }
        return Err(format!(
            "immutable Runtime artifact digest mismatch: expected={content_digest} observed={observed} path={}",
            path.display()
        ));
    }
    Ok(PreparedRuntimeArtifact {
        path,
        content_digest,
        was_present,
    })
}

pub async fn discard_prepared_runtime_artifact(
    artifact: &PreparedRuntimeArtifact,
) -> Result<(), String> {
    if artifact.was_present {
        return Ok(());
    }
    match tokio::fs::remove_file(&artifact.path).await {
        Ok(()) => {
            if let Some(parent) = artifact.path.parent() {
                let _ = tokio::fs::remove_dir(parent).await;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cleanup immutable Runtime candidate {}: {error}",
            artifact.path.display()
        )),
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_staging.rs"]
mod artifact_staging_tests;
