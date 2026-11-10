// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Installation-only predecessor admission for atomic protocol replacement.

use std::path::{Path, PathBuf};

use super::BoundBundleRegistrationAdmission;
use super::VerifiedRuntimeArtifactBoundEnvelope;
use super::verify_runtime_artifact_bound_bundle_envelope;

/// A predecessor whose registration identities and artifact bindings are
/// intact, but whose registration digests may require provider replacement in
/// the same protocol publication transaction.
pub(crate) struct VerifiedRuntimeArtifactBoundBinaryReplacementPredecessor {
    envelope: VerifiedRuntimeArtifactBoundEnvelope,
}

impl VerifiedRuntimeArtifactBoundBinaryReplacementPredecessor {
    pub(crate) fn root(&self) -> &Path {
        &self.envelope.root
    }

    pub(crate) fn members(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::blake3_content_digest::Blake3ContentDigest>
    {
        &self.envelope.members
    }

    pub(crate) fn bundle_digest(&self) -> &crate::blake3_content_digest::Blake3ContentDigest {
        &self.envelope.bundle_digest
    }

    pub(crate) fn member_path(&self, member: &str) -> Option<PathBuf> {
        self.envelope
            .members
            .contains_key(member)
            .then(|| self.envelope.root.join(member))
    }
}

pub(crate) fn verify_runtime_artifact_bound_bundle_for_binary_replacement(
    candidate: &Path,
) -> Result<VerifiedRuntimeArtifactBoundBinaryReplacementPredecessor, String> {
    verify_runtime_artifact_bound_bundle_envelope(
        candidate,
        BoundBundleRegistrationAdmission::BinaryReplacement,
    )
    .map(|envelope| VerifiedRuntimeArtifactBoundBinaryReplacementPredecessor { envelope })
}
