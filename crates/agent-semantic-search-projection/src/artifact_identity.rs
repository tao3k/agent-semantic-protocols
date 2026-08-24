//! Content identities shared by Search publishers and materializers.

/// Content address for the immutable source-index projection of one snapshot.
///
/// Database adapters may persist an artifact under this identity, but the
/// identity remains independent of a storage engine.
#[must_use]
pub fn source_index_artifact_digest(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> String {
    agent_semantic_content_identity::hash_derived_artifact_key(
        agent_semantic_content_identity::DerivedArtifactKeyInput {
            artifact_kind: "source-index",
            schema_id: "asp.source-index-artifact.v1",
            snapshot_root: &source_snapshot.root_digest,
            provider_digest: &source_snapshot.provider_digest,
            parameters: &[],
        },
    )
    .value
}
