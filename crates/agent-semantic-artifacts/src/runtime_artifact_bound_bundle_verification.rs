// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Verification for complete execution-closure-bound Runtime bundles.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{RuntimeArtifactBundleBinding, runtime_artifact_bound_bundle_digest};
use crate::runtime_artifact_store::runtime_artifact_content_digest;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeArtifactBoundBundleManifest {
    schema_id: String,
    schema_version: u64,
    bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    members: std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    execution_binding: RuntimeArtifactBundleBinding,
}

/// A serving-admissible Runtime bundle whose executable members and complete
/// execution closure are proven by one immutable manifest digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeArtifactBoundBundle {
    envelope: VerifiedRuntimeArtifactBoundEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRuntimeArtifactBoundEnvelope {
    pub(crate) root: PathBuf,
    pub(crate) bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub(crate) members:
        std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>,
    pub(crate) execution_binding: RuntimeArtifactBundleBinding,
}

pub(crate) enum BoundBundleRegistrationAdmission<'a> {
    Strict,
    ProviderReplacement(&'a str),
    BinaryReplacement,
}

impl VerifiedRuntimeArtifactBoundBundle {
    #[must_use]
    pub fn bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.envelope.bundle_digest
    }

    #[must_use]
    pub fn execution_binding(&self) -> &RuntimeArtifactBundleBinding {
        &self.envelope.execution_binding
    }

    #[must_use]
    pub fn member_path(&self, member: &str) -> Option<PathBuf> {
        self.envelope
            .members
            .contains_key(member)
            .then(|| self.envelope.root.join(member))
    }

    #[must_use]
    pub fn member_digest(
        &self,
        member: &str,
    ) -> Option<&crate::blake3_content_digest::Blake3ContentDigest> {
        self.envelope.members.get(member)
    }

    #[must_use]
    pub fn members(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>
    {
        &self.envelope.members
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
    verify_runtime_artifact_bound_bundle_envelope(
        candidate,
        BoundBundleRegistrationAdmission::Strict,
    )
    .map(|envelope| VerifiedRuntimeArtifactBoundBundle { envelope })
}

pub(crate) fn verify_runtime_artifact_bound_bundle_envelope(
    candidate: &Path,
    registration_admission: BoundBundleRegistrationAdmission<'_>,
) -> Result<VerifiedRuntimeArtifactBoundEnvelope, String> {
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
    let closure = crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_materialized_members(
        candidate,
        &manifest.members,
    )?;
    if closure.binding()? != manifest.execution_binding {
        return Err(
            "reasonKind=runtime-execution-closure-binding-drift Runtime execution closure bytes do not reproduce the manifest binding"
                .to_owned(),
        );
    }
    match registration_admission {
        BoundBundleRegistrationAdmission::Strict => {
            closure.validate_against_bundle(&manifest.members)?
        }
        BoundBundleRegistrationAdmission::ProviderReplacement(replaced_provider_id) => closure
            .validate_provider_replacement_predecessor(&manifest.members, replaced_provider_id)?,
        BoundBundleRegistrationAdmission::BinaryReplacement => {
            closure.validate_binary_replacement_predecessor(&manifest.members)?
        }
    }
    Ok(VerifiedRuntimeArtifactBoundEnvelope {
        root: candidate.to_path_buf(),
        bundle_digest: derived,
        members: manifest.members,
        execution_binding: manifest.execution_binding,
    })
}
