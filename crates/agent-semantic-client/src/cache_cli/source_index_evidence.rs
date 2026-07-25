//! Artifact evidence projection for cache-owned source-index operations.

pub(super) fn source_index_refresh_artifact_evidence(
    report: &crate::source_index::SourceIndexRefreshReport,
) -> agent_semantic_content_identity::DerivedSourceArtifactEvidence {
    agent_semantic_content_identity::DerivedSourceArtifactEvidence::current(
        agent_semantic_content_identity::DerivedSourceArtifactKind::SourceIndex,
        report.index_artifact_digest(),
        report.source_snapshot().clone(),
    )
}

pub(super) fn source_index_lookup_artifact_evidence(
    result: &crate::source_index::SourceIndexLookupResult,
) -> Option<agent_semantic_content_identity::DerivedSourceArtifactEvidence> {
    Some(
        agent_semantic_content_identity::DerivedSourceArtifactEvidence::current(
            agent_semantic_content_identity::DerivedSourceArtifactKind::SourceIndex,
            result.index_artifact_digest.as_deref()?,
            result.source_snapshot.clone()?,
        ),
    )
}
