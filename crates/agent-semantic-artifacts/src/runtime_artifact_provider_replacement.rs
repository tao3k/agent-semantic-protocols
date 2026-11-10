// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Installation-only predecessor admission for atomic Provider replacement.

use std::path::Path;

use super::BoundBundleRegistrationAdmission;
use super::VerifiedRuntimeArtifactBoundEnvelope;
use super::verify_runtime_artifact_bound_bundle_envelope;

/// A predecessor that may enter only the single-Provider replacement path.
/// It cannot enter serving or active-provider projection.
pub(crate) struct VerifiedRuntimeArtifactBoundProviderReplacementPredecessor {
    envelope: VerifiedRuntimeArtifactBoundEnvelope,
}

impl VerifiedRuntimeArtifactBoundProviderReplacementPredecessor {
    pub(crate) fn bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.envelope.bundle_digest
    }

    pub(crate) fn members(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>
    {
        &self.envelope.members
    }
}

/// Verify an immutable predecessor for replacement of exactly one Provider.
/// Only that Provider's registration digest may differ from the current
/// built-in registry; all other bindings remain exact.
pub(crate) async fn verify_runtime_artifact_bound_bundle_for_provider_replacement(
    candidate: &Path,
    replaced_provider_id: &str,
) -> Result<VerifiedRuntimeArtifactBoundProviderReplacementPredecessor, String> {
    let candidate = candidate.to_path_buf();
    let replaced_provider_id = replaced_provider_id.to_owned();
    tokio::task::spawn_blocking(move || {
        verify_runtime_artifact_bound_bundle_envelope(
            &candidate,
            BoundBundleRegistrationAdmission::ProviderReplacement(replaced_provider_id.as_str()),
        )
        .map(|envelope| VerifiedRuntimeArtifactBoundProviderReplacementPredecessor { envelope })
    })
    .await
    .map_err(|error| {
        format!("verify Runtime Provider-replacement predecessor task failed: {error}")
    })?
}
