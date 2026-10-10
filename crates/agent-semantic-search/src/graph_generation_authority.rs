// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Active workspace-generation admission for graph consumers.

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceError;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;

/// A graph input whose source snapshot and complete generation have both been
/// proven to match the active workspace generation.
#[derive(Debug)]
pub struct AdmittedGraphGenerationV1<'a> {
    source_snapshot: &'a SourceSnapshotEvidence,
    generation: &'a WorkspaceGenerationEvidenceV1,
}

impl<'a> AdmittedGraphGenerationV1<'a> {
    /// Validate a graph snapshot and its candidate generation against the
    /// active complete workspace generation.
    pub fn admit(
        source_snapshot: &'a SourceSnapshotEvidence,
        candidate: &'a ValidatedWorkspaceGenerationV1,
        active: &ValidatedWorkspaceGenerationV1,
    ) -> Result<Self, WorkspaceGenerationEvidenceError> {
        candidate.validate_same_generation(active)?;
        let candidate = candidate.evidence();
        if source_snapshot.root_digest != candidate.root_digest
            || source_snapshot.leaf_count as u64 != candidate.leaf_count
        {
            return Err(
                WorkspaceGenerationEvidenceError::SnapshotGenerationMismatch {
                    snapshot_root: source_snapshot.root_digest.clone(),
                    generation_root: candidate.root_digest.clone(),
                    snapshot_leaf_count: source_snapshot.leaf_count as u64,
                    generation_leaf_count: candidate.leaf_count,
                },
            );
        }
        Ok(Self {
            source_snapshot,
            generation: candidate,
        })
    }

    /// Return the snapshot proven to belong to the admitted generation.
    pub fn source_snapshot(&self) -> &'a SourceSnapshotEvidence {
        self.source_snapshot
    }

    /// Return the complete generation evidence used for admission.
    pub fn generation(&self) -> &'a WorkspaceGenerationEvidenceV1 {
        self.generation
    }
}
