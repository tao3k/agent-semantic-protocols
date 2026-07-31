//! Artifact evidence projection for cache-owned source-index operations.

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
