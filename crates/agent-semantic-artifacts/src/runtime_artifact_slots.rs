// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Active/healthy bundle slots and immutable candidate materialization.

use std::path::Path;
use std::path::PathBuf;

#[path = "runtime_artifact_slot_links.rs"]
mod slot_links;

pub(crate) use slot_links::publish_runtime_artifact_slot;
pub(crate) use slot_links::publish_runtime_artifact_slot_under_guard;
pub(crate) use slot_links::read_runtime_artifact_slot;
pub(crate) use slot_links::read_runtime_artifact_slot_under_guard;
pub(crate) use slot_links::restore_runtime_artifact_slot;
pub(crate) use slot_links::restore_runtime_artifact_slot_under_guard;

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
    evaluator_abi_digest: crate::blake3_content_digest::Blake3ContentDigest,
    schema_bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
}

impl RuntimeArtifactBundleBinding {
    #[must_use]
    pub fn new(
        provider_registration_digest: crate::blake3_content_digest::Blake3ContentDigest,
        provider_artifact_set_digest: crate::blake3_content_digest::Blake3ContentDigest,
        evaluator_policy_digest: crate::blake3_content_digest::Blake3ContentDigest,
        evaluator_abi_digest: crate::blake3_content_digest::Blake3ContentDigest,
        schema_bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    ) -> Self {
        Self {
            schema_id: "agent.semantic-protocols.runtime-artifact-bundle-binding".to_owned(),
            schema_version: "2".to_owned(),
            provider_registration_digest,
            provider_artifact_set_digest,
            evaluator_policy_digest,
            evaluator_abi_digest,
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
    pub fn evaluator_abi_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.evaluator_abi_digest
    }

    #[must_use]
    pub fn schema_bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.schema_bundle_digest
    }

    /// Stable provider-catalog product identity used by Runtime execution
    /// bindings. Paths, activation sequence numbers, and filesystem metadata
    /// are deliberately absent.
    #[must_use]
    pub fn provider_catalog_digest(&self) -> crate::blake3_content_digest::Blake3ContentDigest {
        let mut identity = Vec::new();
        identity.extend_from_slice(b"agent.semantic-protocols.runtime-provider-catalog.v1\0");
        for digest in [
            &self.provider_registration_digest,
            &self.provider_artifact_set_digest,
        ] {
            identity.extend_from_slice(digest.as_str().as_bytes());
            identity.push(0);
        }
        crate::blake3_content_digest::Blake3ContentDigest::from_bytes(&identity)
    }

    /// Validate the immutable non-executable inputs carried by a bound bundle.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != "agent.semantic-protocols.runtime-artifact-bundle-binding"
            || self.schema_version != "2"
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
            ("evaluator-abi.json", &self.evaluator_abi_digest),
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
            ("evaluator-abi", &self.evaluator_abi_digest),
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
    pub(super) root: PathBuf,
    pub(super) artifact_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeArtifactBundleManifest {
    pub(super) schema_id: String,
    pub(super) schema_version: u64,
    pub(super) bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub(super) members:
        std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
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
    identity.extend_from_slice(b"agent.semantic-protocols.runtime-binary-bundle-bound.v2\0");
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
        "schemaVersion": 2,
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
    if value.get("executionBinding").is_some() {
        verify_runtime_artifact_bound_bundle(candidate)
            .await
            .map(|bundle| bundle.bundle_digest)
    } else {
        verify_runtime_artifact_bundle(candidate)
            .await
            .map(|bundle| bundle.bundle_digest)
    }
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
    pub fn member_digest(
        &self,
        member: &str,
    ) -> Option<&crate::blake3_content_digest::Blake3ContentDigest> {
        self.members.get(member)
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
    let candidate = candidate.to_path_buf();
    tokio::task::spawn_blocking(move || verify_runtime_artifact_bound_bundle_blocking(&candidate))
        .await
        .map_err(|error| format!("verify Runtime artifact bound bundle task failed: {error}"))?
}

/// Blocking form for startup and installation boundaries that are not inside
/// an async request path.
pub fn verify_runtime_artifact_bound_bundle_blocking(
    candidate: &Path,
) -> Result<VerifiedRuntimeArtifactBoundBundle, String> {
    let manifest_path = candidate.join("bundle.json");
    let bytes = std::fs::read(&manifest_path).map_err(|error| {
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
        || manifest.schema_version != 2
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
        let observed_digest = runtime_artifact_content_digest(&artifact)?;
        if &observed_digest != expected_digest {
            return Err(format!(
                "Runtime artifact bundle member digest mismatch: member={member} expected={expected_digest} observed={observed_digest}"
            ));
        }
    }
    let closure =
        crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_materialized_members(
            candidate,
            &manifest.members,
        )?;
    if closure.binding()? != manifest.execution_binding {
        return Err(
            "reasonKind=runtime-execution-closure-binding-drift Runtime execution closure bytes do not reproduce the manifest binding"
                .to_owned(),
        );
    }
    closure.validate_against_bundle(&manifest.members)?;
    Ok(VerifiedRuntimeArtifactBoundBundle {
        root: candidate.to_path_buf(),
        bundle_digest: derived,
        members: manifest.members,
        execution_binding: manifest.execution_binding,
    })
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
#[path = "../tests/unit/runtime_artifact_bundle_binding.rs"]
mod runtime_artifact_bundle_binding_tests;

#[cfg(test)]
#[path = "../tests/unit/runtime_artifact_staging.rs"]
mod artifact_staging_tests;
