// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed provider projection from the single verified active Runtime bundle.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRuntimeProviderArtifact {
    pub language_id: String,
    pub provider_id: String,
    pub materialized_path: PathBuf,
    pub artifact_digest: String,
}

/// Serving-only provider set admitted from a complete bound Runtime bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundActiveRuntimeProviderSet {
    pub runtime_bundle_digest: String,
    pub execution_binding: crate::runtime_artifact_slots::RuntimeArtifactBundleBinding,
    pub providers: Vec<ActiveRuntimeProviderArtifact>,
}

/// Installation-only projection used to assemble an atomic protocol and
/// Provider successor. This projection cannot enter serving dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryReplacementRuntimeProviderArtifact {
    pub language_id: String,
    pub provider_id: String,
    pub materialized_path: PathBuf,
    pub artifact_digest: String,
    pub registration_digest_matches_current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryReplacementRuntimeProviderSet {
    pub runtime_bundle_path: PathBuf,
    pub runtime_bundle_digest: crate::blake3_content_digest::Blake3ContentDigest,
    pub providers: Vec<BinaryReplacementRuntimeProviderArtifact>,
}

pub fn load_active_runtime_bound_provider_set(
    state_home: &Path,
) -> Result<BoundActiveRuntimeProviderSet, String> {
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "state=runtime-provider-artifacts-unavailable reasonKind=runtime-active-generation-unavailable path={} error={error}",
            layout.active_slot().display()
        )
    })?;
    let bundle =
        crate::runtime_artifact_slots::verify_runtime_artifact_bound_bundle_blocking(&active)?;
    bound_provider_set_from_verified_bundle(bundle)
}

pub async fn load_active_runtime_bound_provider_set_async(
    state_home: &Path,
) -> Result<BoundActiveRuntimeProviderSet, String> {
    let state_home = state_home.to_path_buf();
    tokio::task::spawn_blocking(move || load_active_runtime_bound_provider_set(&state_home))
        .await
        .map_err(|error| format!("load active Runtime provider set task failed: {error}"))?
}

pub fn load_active_runtime_bound_provider_set_for_binary_replacement(
    state_home: &Path,
) -> Result<BinaryReplacementRuntimeProviderSet, String> {
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "state=runtime-provider-artifacts-unavailable reasonKind=runtime-active-generation-unavailable path={} error={error}",
            layout.active_slot().display()
        )
    })?;
    let bundle =
        crate::runtime_artifact_slots::verify_runtime_artifact_bound_bundle_for_binary_replacement(
            &active,
        )?;
    let closure = crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_materialized_members(
        bundle.root(),
        bundle.members(),
    )?;
    let registrations = agent_semantic_provider_protocol::builtin_provider_registrations()?
        .into_iter()
        .map(|registration| (registration.provider_id.clone(), registration))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut providers = Vec::with_capacity(closure.provider_registration.entries.len());
    for entry in closure.provider_registration.entries {
        let registration = registrations.get(&entry.provider_id).ok_or_else(|| {
            format!(
                "reasonKind=runtime-execution-closure-provider-registration-unknown providerId={}",
                entry.provider_id
            )
        })?;
        let expected_registration_digest =
            crate::blake3_content_digest::Blake3ContentDigest::from_bytes(
                &serde_json::to_vec(&registration.registration)
                    .map_err(|error| format!("encode built-in provider registration: {error}"))?,
            );
        let artifact_digest = bundle
            .members()
            .get(&entry.artifact_member)
            .ok_or_else(|| {
                format!(
                    "verified Runtime member vanished: {}",
                    entry.artifact_member
                )
            })?;
        providers.push(BinaryReplacementRuntimeProviderArtifact {
            language_id: entry.language_id,
            provider_id: entry.provider_id,
            materialized_path: bundle.member_path(&entry.artifact_member).ok_or_else(|| {
                format!(
                    "verified Runtime member vanished: {}",
                    entry.artifact_member
                )
            })?,
            artifact_digest: artifact_digest.to_string(),
            registration_digest_matches_current: entry.registration_digest
                == expected_registration_digest,
        });
    }
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    Ok(BinaryReplacementRuntimeProviderSet {
        runtime_bundle_path: active,
        runtime_bundle_digest: bundle.bundle_digest().clone(),
        providers,
    })
}

fn bound_provider_set_from_verified_bundle(
    bundle: crate::runtime_artifact_slots::VerifiedRuntimeArtifactBoundBundle,
) -> Result<BoundActiveRuntimeProviderSet, String> {
    let mut providers = Vec::new();
    for registration in agent_semantic_provider_protocol::builtin_provider_registrations()? {
        let member_name = registration.provider_id.as_str();
        let Some(member_digest) = bundle.member_digest(member_name) else {
            continue;
        };
        let materialized_path = bundle
            .member_path(member_name)
            .ok_or_else(|| format!("verified Runtime member vanished: {member_name}"))?;
        providers.push(ActiveRuntimeProviderArtifact {
            language_id: registration.language_id,
            provider_id: registration.provider_id,
            materialized_path,
            artifact_digest: member_digest.to_string(),
        });
    }
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    Ok(BoundActiveRuntimeProviderSet {
        runtime_bundle_digest: bundle.bundle_digest().to_string(),
        execution_binding: bundle.execution_binding().clone(),
        providers,
    })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_active_provider_set.rs"]
mod tests;
