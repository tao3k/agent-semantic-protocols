// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Active/healthy bundle slots and immutable candidate materialization.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

#[path = "runtime_artifact_slot_links.rs"]
mod slot_links;

use slot_links::publish_runtime_artifact_slot;
use slot_links::publish_runtime_artifact_slot_under_guard;
use slot_links::read_runtime_artifact_slot;
use slot_links::read_runtime_artifact_slot_under_guard;
use slot_links::restore_runtime_artifact_slot;
use slot_links::restore_runtime_artifact_slot_under_guard;

use serde::Deserialize;

use crate::runtime_artifact_store::runtime_artifact_content_digest;
use crate::runtime_artifact_store::stage_runtime_artifact;

/// Immutable content identity for every non-executable input that can change
/// Runtime search, query, or evaluator behavior. This value is part of the
/// bundle digest; it is never reconstructed from mutable State Home files.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArtifactBundleBinding {
    schema_id: String,
    schema_version: String,
    provider_registration_digest: crate::blake3_content_digest::Blake3ContentDigest,
    provider_artifact_set_digest: crate::blake3_content_digest::Blake3ContentDigest,
    evaluator_policy_digest: crate::blake3_content_digest::Blake3ContentDigest,
    schema_bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
}

impl RuntimeArtifactBundleBinding {
    #[must_use]
    pub fn new(
        provider_registration_digest: crate::blake3_content_digest::Blake3ContentDigest,
        provider_artifact_set_digest: crate::blake3_content_digest::Blake3ContentDigest,
        evaluator_policy_digest: crate::blake3_content_digest::Blake3ContentDigest,
        schema_bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    ) -> Self {
        Self {
            schema_id: "agent.semantic-protocols.runtime-artifact-bundle-binding".to_owned(),
            schema_version: "1".to_owned(),
            provider_registration_digest,
            provider_artifact_set_digest,
            evaluator_policy_digest,
            schema_bundle_digest,
        }
    }

    #[must_use]
    pub fn provider_registration_digest(
        &self,
    ) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.provider_registration_digest
    }

    #[must_use]
    pub fn provider_artifact_set_digest(
        &self,
    ) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.provider_artifact_set_digest
    }

    #[must_use]
    pub fn evaluator_policy_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.evaluator_policy_digest
    }

    #[must_use]
    pub fn schema_bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.schema_bundle_digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_id != "agent.semantic-protocols.runtime-artifact-bundle-binding"
            || self.schema_version != "1"
        {
            return Err(
                "reasonKind=runtime-bundle-binding-schema-mismatch Runtime bundle binding schema identity is invalid"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn validate_materialized_members(
        &self,
        members: &std::collections::BTreeMap<
            String,
            crate::blake3_content_digest::Blake3ContentDigest,
        >,
    ) -> Result<(), String> {
        for (member, expected) in [
            (
                "provider-registration.json",
                &self.provider_registration_digest,
            ),
            ("provider-artifact-set", &self.provider_artifact_set_digest),
            ("evaluator-policy.json", &self.evaluator_policy_digest),
            ("schema-bundle.json", &self.schema_bundle_digest),
        ] {
            match members.get(member) {
                Some(observed) if observed == expected => {}
                Some(observed) => {
                    return Err(format!(
                        "reasonKind=runtime-bundle-binding-member-drift member={member} expected={expected} observed={observed}"
                    ));
                }
                None => {
                    return Err(format!(
                        "reasonKind=runtime-bundle-binding-member-missing member={member}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn append_identity_bytes(&self, identity: &mut Vec<u8>) {
        identity.extend_from_slice(self.schema_id.as_bytes());
        identity.push(0);
        identity.extend_from_slice(self.schema_version.as_bytes());
        identity.push(0);
        for (field, digest) in [
            ("provider-registration", &self.provider_registration_digest),
            ("provider-artifact-set", &self.provider_artifact_set_digest),
            ("evaluator-policy", &self.evaluator_policy_digest),
            ("schema-bundle", &self.schema_bundle_digest),
        ] {
            identity.extend_from_slice(field.as_bytes());
            identity.push(0);
            identity.extend_from_slice(digest.as_str().as_bytes());
            identity.push(0);
        }
    }
}

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeArtifactBoundBundleManifest {
    schema_id: String,
    schema_version: u64,
    bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    members: std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    execution_binding: RuntimeArtifactBundleBinding,
}

/// A content-proven immutable Runtime bundle.  Consumers may resolve optional
/// capability executables only through this authority; state-home descriptors
/// and PATH lookup are deliberately outside the model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeArtifactBundle {
    root: PathBuf,
    bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    members: std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
}

impl VerifiedRuntimeArtifactBundle {
    pub fn bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.bundle_digest
    }

    pub fn member_path(&self, member: &str) -> Option<PathBuf> {
        self.members
            .contains_key(member)
            .then(|| self.root.join(member))
    }

    pub fn member_digest(
        &self,
        member: &str,
    ) -> Option<&crate::blake3_content_digest::Blake3ContentDigest> {
        self.members.get(member)
    }

    /// Complete immutable member index used when a single capability is
    /// replaced by republishing the whole Runtime generation.
    pub fn members(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>
    {
        &self.members
    }
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

/// Derive the product identity of a Runtime bundle from executable members and
/// its complete execution closure. Publication sequence numbers intentionally
/// do not participate in this digest.
pub fn runtime_artifact_bound_bundle_digest(
    members: &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    binding: &RuntimeArtifactBundleBinding,
) -> crate::blake3_content_digest::Blake3ContentDigest {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"agent.semantic-protocols.runtime-binary-bundle-bound.v1\0");
    for (member, digest) in members {
        identity.extend_from_slice(member.as_bytes());
        identity.push(0);
        identity.extend_from_slice(digest.as_str().as_bytes());
        identity.push(0);
    }
    binding.append_identity_bytes(&mut identity);
    crate::blake3_content_digest::Blake3ContentDigest::from_bytes(&identity)
}

/// Stage the one manifest accepted by strict Runtime serving admission. The
/// candidate directory is not a serving authority until the Artifacts CAS
/// publishes it through the active slot.
pub fn stage_runtime_artifact_bound_bundle_manifest(
    candidate: &Path,
    members: &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    execution_binding: &RuntimeArtifactBundleBinding,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    if members.is_empty() {
        return Err("Runtime artifact bound bundle members are empty".to_owned());
    }
    execution_binding.validate()?;
    execution_binding.validate_materialized_members(members)?;
    let bundle_digest = runtime_artifact_bound_bundle_digest(members, execution_binding);
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
        "schemaVersion": 1,
        "bundleDigest": bundle_digest,
        "members": members,
        "executionBinding": execution_binding,
    }))
    .map_err(|error| format!("encode Runtime artifact bound bundle manifest: {error}"))?;
    let manifest_path = candidate.join("bundle.json");
    let temporary = candidate.join(format!(".bundle.{}.tmp", std::process::id()));
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "stage Runtime artifact bound bundle manifest {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &manifest_path).map_err(|error| {
        format!(
            "publish Runtime artifact bound bundle manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    Ok(bundle_digest)
}

pub async fn runtime_artifact_candidate_bundle_digest(
    candidate: &Path,
) -> Result<crate::blake3_content_digest::Blake3ContentDigest, String> {
    verify_runtime_artifact_bundle(candidate)
        .await
        .map(|bundle| bundle.bundle_digest)
}

pub async fn verify_runtime_artifact_bundle(
    candidate: &Path,
) -> Result<VerifiedRuntimeArtifactBundle, String> {
    let candidate = candidate.to_path_buf();
    tokio::task::spawn_blocking(move || verify_runtime_artifact_bundle_blocking(&candidate))
        .await
        .map_err(|error| format!("verify Runtime artifact bundle task failed: {error}"))?
}

/// Verify one immutable Runtime generation without entering an async runtime.
///
/// Lifecycle probes use this form while holding no Runtime locks. It has the
/// same manifest and member-content admission rules as the async API.
pub fn verify_runtime_artifact_bundle_blocking(
    candidate: &Path,
) -> Result<VerifiedRuntimeArtifactBundle, String> {
    let manifest_path = candidate.join("bundle.json");
    let bytes = std::fs::read(&manifest_path).map_err(|error| {
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
    for (member, expected_digest) in &manifest.members {
        let member_path = Path::new(&member);
        if member_path.components().count() != 1 || member == "." || member == ".." {
            return Err(format!("invalid Runtime artifact bundle member `{member}`"));
        }
        let artifact = candidate.join(member);
        let observed_digest = runtime_artifact_content_digest(&artifact)?;
        if &observed_digest != expected_digest {
            return Err(format!(
                "Runtime artifact bundle member digest mismatch: member={member} expected={expected_digest} observed={observed_digest}"
            ));
        }
    }
    Ok(VerifiedRuntimeArtifactBundle {
        root: candidate.to_path_buf(),
        bundle_digest: derived,
        members: manifest.members,
    })
}

/// A serving-admissible Runtime bundle whose executable members and complete
/// execution closure are proven by one immutable manifest digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeArtifactBoundBundle {
    root: PathBuf,
    bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    members: std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    execution_binding: RuntimeArtifactBundleBinding,
}

impl VerifiedRuntimeArtifactBoundBundle {
    #[must_use]
    pub fn bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.bundle_digest
    }

    #[must_use]
    pub fn execution_binding(&self) -> &RuntimeArtifactBundleBinding {
        &self.execution_binding
    }

    #[must_use]
    pub fn member_path(&self, member: &str) -> Option<PathBuf> {
        self.members
            .contains_key(member)
            .then(|| self.root.join(member))
    }

    #[must_use]
    pub fn members(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>
    {
        &self.members
    }
}

/// Strict serving admission for the content-addressed Runtime architecture.
/// Unbound members-only manifests are observable migration inputs, never valid
/// serving authorities.
pub async fn verify_runtime_artifact_bound_bundle(
    candidate: &Path,
) -> Result<VerifiedRuntimeArtifactBoundBundle, String> {
    let manifest_path = candidate.join("bundle.json");
    let bytes = tokio::fs::read(&manifest_path).await.map_err(|error| {
        format!(
            "read Runtime artifact bundle manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "decode Runtime artifact bundle manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    if value.get("executionBinding").is_none() {
        return Err(
            "reasonKind=runtime-bundle-binding-missing Runtime bundle manifest has no execution binding"
                .to_owned(),
        );
    }
    let manifest: RuntimeArtifactBoundBundleManifest = serde_json::from_value(value).map_err(
        |error| {
            format!(
                "reasonKind=runtime-bundle-binding-invalid decode Runtime artifact bound bundle manifest {}: {error}",
                manifest_path.display()
            )
        },
    )?;
    if manifest.schema_id != "agent.semantic-protocols.runtime-binary-bundle"
        || manifest.schema_version != 1
        || manifest.members.is_empty()
    {
        return Err("invalid Runtime artifact bound bundle manifest authority".to_owned());
    }
    manifest.execution_binding.validate()?;
    let derived =
        runtime_artifact_bound_bundle_digest(&manifest.members, &manifest.execution_binding);
    if derived != manifest.bundle_digest {
        return Err(
            "reasonKind=runtime-bundle-binding-digest-mismatch Runtime artifact bound bundle manifest digest mismatch"
                .to_owned(),
        );
    }
    manifest
        .execution_binding
        .validate_materialized_members(&manifest.members)?;
    for (member, expected_digest) in &manifest.members {
        let member_path = Path::new(member);
        if member_path.components().count() != 1 || member == "." || member == ".." {
            return Err(format!("invalid Runtime artifact bundle member `{member}`"));
        }
        let artifact = candidate.join(member);
        let observed_digest = runtime_artifact_candidate_digest(&artifact).await?;
        if &observed_digest != expected_digest {
            return Err(format!(
                "Runtime artifact bundle member digest mismatch: member={member} expected={expected_digest} observed={observed_digest}"
            ));
        }
    }
    Ok(VerifiedRuntimeArtifactBoundBundle {
        root: candidate.to_path_buf(),
        bundle_digest: derived,
        members: manifest.members,
        execution_binding: manifest.execution_binding,
    })
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRuntimeArtifact {
    pub path: PathBuf,
    pub content_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub was_present: bool,
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
    let _state_home = state_home.to_path_buf();
    let candidate_dir = candidate_dir.to_path_buf();
    let source = source.to_path_buf();
    let artifact_kind = artifact_kind.to_owned();
    tokio::task::spawn_blocking(move || {
        prepare_runtime_artifact_candidate_blocking(
            &_state_home,
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
    let _state_home = state_home.to_path_buf();
    let candidate_dir = candidate_dir.to_path_buf();
    let bytes = bytes.to_vec();
    let artifact_kind = artifact_kind.to_owned();
    tokio::task::spawn_blocking(move || {
        let content_digest = crate::blake3_content_digest::Blake3ContentDigest::from_bytes(&bytes);
        let path = candidate_dir.join(&artifact_kind);
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
    _state_home: &Path,
    candidate_dir: &Path,
    source: &Path,
    artifact_kind: &str,
) -> Result<PreparedRuntimeArtifact, String> {
    let content_digest = runtime_artifact_content_digest(source)?;
    let path = candidate_dir.join(artifact_kind);
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

#[cfg(test)]
mod runtime_artifact_bundle_binding_tests {
    use super::*;

    fn digest(label: &str) -> crate::blake3_content_digest::Blake3ContentDigest {
        crate::blake3_content_digest::Blake3ContentDigest::from_bytes(label.as_bytes())
    }

    #[test]
    fn bundle_identity_covers_the_complete_runtime_execution_closure() {
        let members = std::collections::BTreeMap::from([("asp".to_owned(), digest("asp"))]);
        let baseline = RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("schema-bundle"),
        );
        let baseline_digest = runtime_artifact_bound_bundle_digest(&members, &baseline);

        for changed in [
            RuntimeArtifactBundleBinding::new(
                digest("provider-registration-v2"),
                digest("provider-artifact-set"),
                digest("evaluator-policy"),
                digest("schema-bundle"),
            ),
            RuntimeArtifactBundleBinding::new(
                digest("provider-registration"),
                digest("provider-artifact-set-v2"),
                digest("evaluator-policy"),
                digest("schema-bundle"),
            ),
            RuntimeArtifactBundleBinding::new(
                digest("provider-registration"),
                digest("provider-artifact-set"),
                digest("evaluator-policy-v2"),
                digest("schema-bundle"),
            ),
            RuntimeArtifactBundleBinding::new(
                digest("provider-registration"),
                digest("provider-artifact-set"),
                digest("evaluator-policy"),
                digest("schema-bundle-v2"),
            ),
        ] {
            assert_ne!(
                runtime_artifact_bound_bundle_digest(&members, &changed),
                baseline_digest,
                "every execution-closure leaf must participate in Runtime bundle identity"
            );
        }
    }

    #[tokio::test]
    async fn bound_bundle_admission_rejects_an_unbound_members_only_manifest() {
        let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
        let candidate = temporary.path().join("candidate");
        std::fs::create_dir_all(&candidate).expect("candidate directory");
        let executable = candidate.join("asp");
        std::fs::write(&executable, b"runtime executable").expect("runtime executable");
        let members = std::collections::BTreeMap::from([(
            "asp".to_owned(),
            runtime_artifact_candidate_digest(&executable)
                .await
                .expect("member digest"),
        )]);
        let unbound_digest = runtime_artifact_bundle_digest(&members);
        std::fs::write(
            candidate.join("bundle.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
                "schemaVersion": 1,
                "bundleDigest": unbound_digest,
                "members": members,
            }))
            .expect("unbound manifest bytes"),
        )
        .expect("unbound manifest");

        let error = verify_runtime_artifact_bound_bundle(&candidate)
            .await
            .expect_err("members-only bundles must not enter bound Runtime admission");
        assert!(error.contains("reasonKind=runtime-bundle-binding-missing"));
    }

    #[tokio::test]
    async fn bound_bundle_admission_returns_the_exact_execution_closure() {
        let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
        let candidate = temporary.path().join("candidate");
        std::fs::create_dir_all(&candidate).expect("candidate directory");
        let executable = candidate.join("asp");
        std::fs::write(&executable, b"runtime executable").expect("runtime executable");
        let mut members = std::collections::BTreeMap::from([(
            "asp".to_owned(),
            runtime_artifact_candidate_digest(&executable)
                .await
                .expect("member digest"),
        )]);
        for (member, contents) in [
            ("provider-registration.json", "provider-registration"),
            ("provider-artifact-set", "provider-artifact-set"),
            ("evaluator-policy.json", "evaluator-policy"),
            ("schema-bundle.json", "schema-bundle"),
        ] {
            std::fs::write(candidate.join(member), contents).expect("closure member");
            members.insert(member.to_owned(), digest(contents));
        }
        let binding = RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("schema-bundle"),
        );
        let bundle_digest =
            stage_runtime_artifact_bound_bundle_manifest(&candidate, &members, &binding)
                .expect("bound manifest");

        let admitted = verify_runtime_artifact_bound_bundle(&candidate)
            .await
            .expect("bound bundle admission");
        assert_eq!(admitted.bundle_digest(), &bundle_digest);
        assert_eq!(
            admitted.execution_binding().provider_registration_digest(),
            &digest("provider-registration")
        );
        assert_eq!(admitted.member_path("asp"), Some(executable));
    }

    #[tokio::test]
    async fn bound_bundle_admission_rejects_a_tampered_execution_closure() {
        let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
        let candidate = temporary.path().join("candidate");
        std::fs::create_dir_all(&candidate).expect("candidate directory");
        let executable = candidate.join("asp");
        std::fs::write(&executable, b"runtime executable").expect("runtime executable");
        let members = std::collections::BTreeMap::from([(
            "asp".to_owned(),
            runtime_artifact_candidate_digest(&executable)
                .await
                .expect("member digest"),
        )]);
        let admitted_binding = RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("schema-bundle"),
        );
        let bundle_digest = runtime_artifact_bound_bundle_digest(&members, &admitted_binding);
        let tampered_binding = RuntimeArtifactBundleBinding::new(
            digest("provider-registration-v2"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("schema-bundle"),
        );
        std::fs::write(
            candidate.join("bundle.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
                "schemaVersion": 1,
                "bundleDigest": bundle_digest,
                "members": members,
                "executionBinding": tampered_binding,
            }))
            .expect("tampered manifest bytes"),
        )
        .expect("tampered manifest");

        let error = verify_runtime_artifact_bound_bundle(&candidate)
            .await
            .expect_err("a changed closure cannot reuse the prior bundle identity");
        assert!(error.contains("reasonKind=runtime-bundle-binding-digest-mismatch"));
    }

    #[tokio::test]
    async fn bound_bundle_admission_requires_materialized_closure_members() {
        let temporary = tempfile::tempdir().expect("Runtime bundle fixture");
        let candidate = temporary.path().join("candidate");
        std::fs::create_dir_all(&candidate).expect("candidate directory");
        let executable = candidate.join("asp");
        std::fs::write(&executable, b"runtime executable").expect("runtime executable");
        let members = std::collections::BTreeMap::from([(
            "asp".to_owned(),
            runtime_artifact_candidate_digest(&executable)
                .await
                .expect("member digest"),
        )]);
        let binding = RuntimeArtifactBundleBinding::new(
            digest("provider-registration"),
            digest("provider-artifact-set"),
            digest("evaluator-policy"),
            digest("schema-bundle"),
        );
        let bundle_digest = runtime_artifact_bound_bundle_digest(&members, &binding);
        std::fs::write(
            candidate.join("bundle.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
                "schemaVersion": 1,
                "bundleDigest": bundle_digest,
                "members": members,
                "executionBinding": binding,
            }))
            .expect("manifest bytes"),
        )
        .expect("manifest");

        let error = verify_runtime_artifact_bound_bundle(&candidate)
            .await
            .expect_err("digest-only closure leaves cannot serve as Runtime content");
        assert!(error.contains("reasonKind=runtime-bundle-binding-member-missing"));
    }
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
