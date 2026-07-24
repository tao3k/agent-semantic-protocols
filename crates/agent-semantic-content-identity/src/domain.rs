//! Stable schema ids, algorithm tokens, and hash domain separators for `content-identity`.

/// Canonical digest algorithm token for artifact identity v1.
pub const HASH_ALGORITHM_BLAKE3: &str = "blake3";
/// Domain separator for raw artifact payload leaves.
pub const LEAF_DOMAIN_V1: &str = "asp.leaf.v1";
/// Domain separator for Merkle artifact nodes.
pub const NODE_DOMAIN_V1: &str = "asp.node.v1";
/// Domain separator for State Core scoped artifact roots.
pub const ROOT_DOMAIN_V1: &str = "asp.root.v1";
/// Domain separator for queryable artifact root edges.
pub const EDGE_DOMAIN_V1: &str = "asp.edge.v1";
/// Domain separator for canonical JSON payload hashes.
pub const JSON_DOMAIN_V1: &str = "asp.normalized-json.v1";
/// Schema id for artifact identity documents.
pub const ARTIFACT_IDENTITY_SCHEMA_ID: &str = "semantic-artifact-identity";
/// Schema version for artifact identity documents.
pub const ARTIFACT_IDENTITY_SCHEMA_VERSION: &str = "1";
