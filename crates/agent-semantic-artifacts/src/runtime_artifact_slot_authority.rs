// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Slot transition operations for Runtime artifact authority.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::runtime_artifact_slots::{
    RuntimeArtifactBundleManifest, RuntimeArtifactSlotAuthority, publish_runtime_artifact_slot,
    publish_runtime_artifact_slot_under_guard, read_runtime_artifact_slot,
    read_runtime_artifact_slot_under_guard, restore_runtime_artifact_slot,
    restore_runtime_artifact_slot_under_guard, runtime_artifact_bundle_digest,
    runtime_artifact_candidate_digest, verify_runtime_artifact_bound_bundle,
};

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
                let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
                    format!(
                        "decode Runtime artifact bundle manifest {}: {error}",
                        manifest_path.display()
                    )
                })?;
                if value.get("executionBinding").is_some() {
                    verify_runtime_artifact_bound_bundle(candidate).await?;
                    return Ok(());
                }
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
        let member_target = candidate_dir.join(member);
        if artifact == member_target {
            if artifact.is_file() {
                return Ok(());
            }
            return Err(format!(
                "Runtime artifact generation member is unavailable: {}",
                artifact.display()
            ));
        }
        publish_runtime_artifact_slot(artifact, &member_target).await
    }
}
