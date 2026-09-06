// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Qualification rules for a resident workspace generation.

/// Evidence collected before a workspace generation becomes query-ready.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationQualificationEvidence {
    /// Whether the projection segment header was decoded and validated.
    pub projection_segment_header_valid: bool,
    /// Source owners discovered by provider-owned project resolution.
    pub candidate_source_owner_count: usize,
    /// Source owners admitted to the resident projection.
    pub admitted_source_owner_count: usize,
    /// Source owners explicitly rejected with typed evidence.
    pub rejected_source_owner_count: usize,
}

impl WorkspaceGenerationQualificationEvidence {
    /// Qualifies one generation for publication or restoration.
    pub fn qualify(self) -> Result<WorkspaceGenerationReadiness, WorkspaceGenerationRejection> {
        if !self.projection_segment_header_valid {
            return Err(WorkspaceGenerationRejection::InvalidProjectionSegmentHeader);
        }

        let classified_source_owner_count = self
            .admitted_source_owner_count
            .checked_add(self.rejected_source_owner_count)
            .ok_or(WorkspaceGenerationRejection::SourceInventoryCountOverflow)?;
        if classified_source_owner_count != self.candidate_source_owner_count {
            return Err(WorkspaceGenerationRejection::IncompleteSourceInventory {
                candidate_source_owner_count: self.candidate_source_owner_count,
                admitted_source_owner_count: self.admitted_source_owner_count,
                rejected_source_owner_count: self.rejected_source_owner_count,
            });
        }

        if self.candidate_source_owner_count > 0 && self.admitted_source_owner_count == 0 {
            return Err(
                WorkspaceGenerationRejection::EmptyProjectionForNonEmptySourceInventory {
                    candidate_source_owner_count: self.candidate_source_owner_count,
                    rejected_source_owner_count: self.rejected_source_owner_count,
                },
            );
        }

        Ok(WorkspaceGenerationReadiness {
            source_owner_count: self.admitted_source_owner_count,
            legitimately_empty: self.candidate_source_owner_count == 0,
        })
    }
}

/// Qualified facts carried into the Ready state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationReadiness {
    /// Number of queryable source owners in the generation.
    pub source_owner_count: usize,
    /// True only when provider project resolution observed no candidate sources.
    pub legitimately_empty: bool,
}

/// Typed reasons that prevent generation publication or restoration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationRejection {
    /// The exact projection segment cannot be decoded by the active Runtime.
    InvalidProjectionSegmentHeader,
    /// Source inventory counts overflowed during qualification.
    SourceInventoryCountOverflow,
    /// Candidate sources were neither admitted nor rejected.
    IncompleteSourceInventory {
        candidate_source_owner_count: usize,
        admitted_source_owner_count: usize,
        rejected_source_owner_count: usize,
    },
    /// A non-empty source inventory was published as an empty projection.
    EmptyProjectionForNonEmptySourceInventory {
        candidate_source_owner_count: usize,
        rejected_source_owner_count: usize,
    },
}

impl std::fmt::Display for WorkspaceGenerationRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProjectionSegmentHeader => {
                formatter.write_str("workspace exact projection segment header is invalid")
            }
            Self::SourceInventoryCountOverflow => {
                formatter.write_str("workspace source inventory count overflow")
            }
            Self::IncompleteSourceInventory {
                candidate_source_owner_count,
                admitted_source_owner_count,
                rejected_source_owner_count,
            } => write!(
                formatter,
                "workspace source inventory is incomplete: candidates={candidate_source_owner_count} admitted={admitted_source_owner_count} rejected={rejected_source_owner_count}"
            ),
            Self::EmptyProjectionForNonEmptySourceInventory {
                candidate_source_owner_count,
                rejected_source_owner_count,
            } => write!(
                formatter,
                "non-empty workspace source inventory produced an empty projection: candidates={candidate_source_owner_count} rejected={rejected_source_owner_count}"
            ),
        }
    }
}

impl std::error::Error for WorkspaceGenerationRejection {}
