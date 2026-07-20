use agent_semantic_content_identity::{
    DerivedArtifactKeyInput, SourceSnapshotEvidence, hash_derived_artifact_key,
};

/// Content-addressed EvidenceGraph entity shard table.
pub const TURSO_ENTITY_TABLE: &str = "asp_graph_artifact_entity";

/// Content-addressed EvidenceGraph edge shard table.
pub const TURSO_EDGE_TABLE: &str = "asp_graph_artifact_edge";

pub(super) const GRAPH_ARTIFACT_SCHEMA_ID: &str = "asp.evidence-graph-artifact.v1";

/// Derive the immutable EvidenceGraph identity for one pinned source snapshot.
pub fn graph_artifact_digest_for_snapshot(source_snapshot: &SourceSnapshotEvidence) -> String {
    hash_derived_artifact_key(DerivedArtifactKeyInput {
        artifact_kind: "evidence-graph",
        schema_id: GRAPH_ARTIFACT_SCHEMA_ID,
        snapshot_root: &source_snapshot.root_digest,
        provider_digest: &source_snapshot.provider_digest,
        parameters: &[],
    })
    .value
}
