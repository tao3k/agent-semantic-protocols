// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Complete workspace-generation identity and graph admission receipts.

use serde::Deserialize;
use serde::Serialize;

/// Canonical authority carried by every consumer of a published workspace
/// generation.
///
/// This is deliberately language-neutral. Providers publish facts into a
/// generation; they do not define generation identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationEvidenceV1 {
    /// Digest of the complete workspace generation root.
    pub root_digest: String,
    /// Depth of the materialized generation Merkle tree.
    pub root_depth: u32,
    /// Number of content leaves committed by the generation.
    pub leaf_count: u64,
    /// Number of source owners covered by the generation.
    pub owner_count: u64,
}

/// Storage projection selected by the active generation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationStorageV1 {
    /// A stable indexed/Merkle projection represented by depth one.
    Merkle,
    /// A load-once resident Memory Search fixture represented by depth zero.
    ResidentMemory,
}

/// Generation evidence validated once at publication or load-once admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedWorkspaceGenerationV1(WorkspaceGenerationEvidenceV1);

impl ValidatedWorkspaceGenerationV1 {
    /// Validate raw receipt evidence once before it enters a query hot path.
    pub fn new(
        evidence: WorkspaceGenerationEvidenceV1,
    ) -> Result<Self, WorkspaceGenerationEvidenceError> {
        evidence.validate_complete()?;
        Ok(Self(evidence))
    }

    /// Borrow the canonical generation evidence.
    pub fn evidence(&self) -> &WorkspaceGenerationEvidenceV1 {
        &self.0
    }

    /// Compare two already-validated generation identities.
    pub fn validate_same_generation(
        &self,
        active: &Self,
    ) -> Result<(), WorkspaceGenerationEvidenceError> {
        if self != active {
            return Err(WorkspaceGenerationEvidenceError::GenerationMismatch {
                candidate_root: self.0.root_digest.clone(),
                active_root: active.0.root_digest.clone(),
            });
        }
        Ok(())
    }
}

/// Mandatory graph authority state carried by source-index acquisitions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum WorkspaceGenerationAuthority {
    Active {
        evidence: WorkspaceGenerationEvidenceV1,
    },
    Unavailable {
        reason_kind: WorkspaceGenerationUnavailableReason,
    },
}

/// Closed reason domain for an unavailable workspace generation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationUnavailableReason {
    /// No active generation has been admitted for the ProjectId/WorkspaceId pair.
    ActiveWorkspaceGenerationRequired,
}

impl WorkspaceGenerationAuthority {
    /// Construct the fail-closed state used before an active generation is
    /// bound to an acquisition.
    pub fn unavailable() -> Self {
        Self::Unavailable {
            reason_kind: WorkspaceGenerationUnavailableReason::ActiveWorkspaceGenerationRequired,
        }
    }

    pub fn admit_graph(
        &self,
        active: &WorkspaceGenerationEvidenceV1,
    ) -> Result<&WorkspaceGenerationEvidenceV1, WorkspaceGenerationEvidenceError> {
        match self {
            Self::Active { evidence } => {
                evidence.validate_same_generation(active)?;
                Ok(evidence)
            }
            Self::Unavailable { .. } => {
                Err(WorkspaceGenerationEvidenceError::GenerationAuthorityMissing)
            }
        }
    }
}

impl WorkspaceGenerationEvidenceV1 {
    /// Validate that this evidence describes a materialized complete
    /// generation.
    pub fn validate_complete(&self) -> Result<(), WorkspaceGenerationEvidenceError> {
        if self.root_digest.trim().is_empty() {
            return Err(WorkspaceGenerationEvidenceError::MissingRootDigest);
        }
        if self.root_depth > 1 {
            return Err(WorkspaceGenerationEvidenceError::UnsupportedRootDepth {
                root_depth: self.root_depth,
            });
        }
        if self.owner_count > self.leaf_count {
            return Err(WorkspaceGenerationEvidenceError::InvalidOwnerCoverage {
                owner_count: self.owner_count,
                leaf_count: self.leaf_count,
            });
        }
        Ok(())
    }

    /// Select the storage projection without changing generation identity.
    pub fn storage(
        &self,
    ) -> Result<WorkspaceGenerationStorageV1, WorkspaceGenerationEvidenceError> {
        self.validate_complete()?;
        Ok(match self.root_depth {
            0 => WorkspaceGenerationStorageV1::ResidentMemory,
            1 => WorkspaceGenerationStorageV1::Merkle,
            _ => unreachable!("validate_complete rejects rootDepth outside {{0,1}}"),
        })
    }

    pub fn validate_same_generation(
        &self,
        active: &Self,
    ) -> Result<(), WorkspaceGenerationEvidenceError> {
        self.validate_complete()?;
        active.validate_complete()?;
        if self != active {
            return Err(WorkspaceGenerationEvidenceError::GenerationMismatch {
                candidate_root: self.root_digest.clone(),
                active_root: active.root_digest.clone(),
            });
        }
        Ok(())
    }
}

/// Typed reasons that prevent graph admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationEvidenceError {
    GenerationAuthorityMissing,
    MissingRootDigest,
    UnsupportedRootDepth {
        root_depth: u32,
    },
    EmptyGeneration,
    InvalidOwnerCoverage {
        owner_count: u64,
        leaf_count: u64,
    },
    GenerationMismatch {
        candidate_root: String,
        active_root: String,
    },
    SnapshotGenerationMismatch {
        snapshot_root: String,
        generation_root: String,
        snapshot_leaf_count: u64,
        generation_leaf_count: u64,
    },
}

impl std::fmt::Display for WorkspaceGenerationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenerationAuthorityMissing => formatter.write_str(
                "state=cold-required reasonKind=active-workspace-generation-required",
            ),
            Self::MissingRootDigest => formatter.write_str(
                "state=cold-required reasonKind=active-workspace-generation-required field=rootDigest",
            ),
            Self::UnsupportedRootDepth { root_depth } => write!(
                formatter,
                "state=generation-invalid reasonKind=unsupported-root-depth rootDepth={root_depth} allowed=0,1"
            ),
            Self::EmptyGeneration => formatter.write_str(
                "state=cold-required reasonKind=active-workspace-generation-required field=leafCount",
            ),
            Self::InvalidOwnerCoverage {
                owner_count,
                leaf_count,
            } => write!(
                formatter,
                "state=cold-required reasonKind=incomplete-workspace-generation ownerCount={owner_count} leafCount={leaf_count}"
            ),
            Self::GenerationMismatch {
                candidate_root,
                active_root,
            } => write!(
                formatter,
                "state=generation-mismatch reasonKind=graph-generation-not-active candidateRoot={candidate_root} activeRoot={active_root}"
            ),
            Self::SnapshotGenerationMismatch {
                snapshot_root,
                generation_root,
                snapshot_leaf_count,
                generation_leaf_count,
            } => write!(
                formatter,
                "state=generation-mismatch reasonKind=graph-snapshot-generation-mismatch snapshotRoot={snapshot_root} generationRoot={generation_root} snapshotLeafCount={snapshot_leaf_count} generationLeafCount={generation_leaf_count}"
            ),
        }
    }
}

impl std::error::Error for WorkspaceGenerationEvidenceError {}
