//! Snapshot-bound evidence for disposable derived source artifacts.

use serde::{Deserialize, Serialize};

use crate::SourceSnapshotEvidence;

/// Shared schema identifier for snapshot-authority artifact evidence.
pub const DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID: &str =
    "asp.derived-source-artifact-evidence.v1";
/// All derived source artifacts are rebuildable and disposable.
pub const DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION: &str = "disposable";

/// Artifact families whose identity is derived from a pinned source snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivedSourceArtifactKind {
    ParserArtifact,
    SourceIndex,
    CompactGraph,
    GraphOwnerRank,
    EvidenceGraph,
}

/// Whether a derived cache artifact is authoritative for the requested snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivedArtifactAuthorityState {
    Current,
    Missing,
    Stale,
}

/// Reproducible evidence for one snapshot-derived, disposable cache artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedSourceArtifactEvidence {
    pub schema_id: String,
    pub artifact_kind: DerivedSourceArtifactKind,
    pub expected_artifact_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_artifact_digest: Option<String>,
    pub authority_state: DerivedArtifactAuthorityState,
    pub cache_disposition: String,
    pub source_snapshot: SourceSnapshotEvidence,
}

impl DerivedSourceArtifactEvidence {
    /// Evidence for an artifact that exactly matches the requested snapshot.
    #[must_use]
    pub fn current(
        artifact_kind: DerivedSourceArtifactKind,
        artifact_digest: impl Into<String>,
        source_snapshot: SourceSnapshotEvidence,
    ) -> Self {
        let artifact_digest = artifact_digest.into();
        Self::new(
            artifact_kind,
            artifact_digest.clone(),
            Some(artifact_digest),
            DerivedArtifactAuthorityState::Current,
            source_snapshot,
        )
    }

    /// Evidence that no artifact has been materialized for the requested snapshot.
    #[must_use]
    pub fn missing(
        artifact_kind: DerivedSourceArtifactKind,
        expected_artifact_digest: impl Into<String>,
        source_snapshot: SourceSnapshotEvidence,
    ) -> Self {
        Self::new(
            artifact_kind,
            expected_artifact_digest.into(),
            None,
            DerivedArtifactAuthorityState::Missing,
            source_snapshot,
        )
    }

    /// Evidence that cached artifacts exist but none matches the requested snapshot.
    #[must_use]
    pub fn stale(
        artifact_kind: DerivedSourceArtifactKind,
        expected_artifact_digest: impl Into<String>,
        source_snapshot: SourceSnapshotEvidence,
    ) -> Self {
        Self::new(
            artifact_kind,
            expected_artifact_digest.into(),
            None,
            DerivedArtifactAuthorityState::Stale,
            source_snapshot,
        )
    }

    fn new(
        artifact_kind: DerivedSourceArtifactKind,
        expected_artifact_digest: String,
        resolved_artifact_digest: Option<String>,
        authority_state: DerivedArtifactAuthorityState,
        source_snapshot: SourceSnapshotEvidence,
    ) -> Self {
        Self {
            schema_id: DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID.to_owned(),
            artifact_kind,
            expected_artifact_digest,
            resolved_artifact_digest,
            authority_state,
            cache_disposition: DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION.to_owned(),
            source_snapshot,
        }
    }
}
