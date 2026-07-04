//! Queryable Merkle artifact root edges.
//!
//! Edge records are the stable handoff between artifact roots and DB/graph
//! implementations. They let Turso, Python, Julia, and render receipts traverse
//! provenance without embedding artifact payloads.

use crate::identity::{ArtifactHash, ArtifactKind, ArtifactRootRef, EDGE_DOMAIN_V1};
use serde::{Deserialize, Serialize};

/// Schema id for queryable artifact root edges.
pub const ARTIFACT_EDGE_SCHEMA_ID: &str = "semantic-artifact-edge";
/// Schema version for queryable artifact root edges.
pub const ARTIFACT_EDGE_SCHEMA_VERSION: &str = "1";

/// Input for building a queryable artifact root edge.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRootEdgeInput {
    /// Semantic edge role such as `howFrom`, `proof`, or `sourceIndexBundle`.
    pub role: String,
    /// Stable ordinal for repeated edge roles.
    pub ordinal: u64,
    /// Parent root in the Merkle artifact forest.
    pub parent: ArtifactRootRef,
    /// Child root in the Merkle artifact forest.
    pub child: ArtifactRootRef,
}

impl ArtifactRootEdgeInput {
    /// Build an artifact root edge input with ordinal zero.
    pub fn new(role: impl Into<String>, parent: ArtifactRootRef, child: ArtifactRootRef) -> Self {
        Self {
            role: role.into(),
            ordinal: 0,
            parent,
            child,
        }
    }

    /// Assign a stable ordinal for repeated edge roles.
    pub fn with_ordinal(mut self, ordinal: u64) -> Self {
        self.ordinal = ordinal;
        self
    }
}

/// Queryable artifact root edge record.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRootEdge {
    /// Schema id for edge records.
    pub schema_id: String,
    /// Schema version for edge records.
    pub schema_version: String,
    /// Stable edge hash.
    pub edge_hash: ArtifactHash,
    /// Semantic edge role.
    pub role: String,
    /// Stable ordinal for repeated edge roles.
    pub ordinal: u64,
    /// Parent root in the Merkle artifact forest.
    pub parent: ArtifactRootRef,
    /// Child root in the Merkle artifact forest.
    pub child: ArtifactRootRef,
}

/// Build a queryable artifact root edge record.
pub fn build_artifact_root_edge(input: ArtifactRootEdgeInput) -> ArtifactRootEdge {
    let edge_hash = hash_artifact_root_edge(&input);
    ArtifactRootEdge {
        schema_id: ARTIFACT_EDGE_SCHEMA_ID.to_string(),
        schema_version: ARTIFACT_EDGE_SCHEMA_VERSION.to_string(),
        edge_hash,
        role: input.role,
        ordinal: input.ordinal,
        parent: input.parent,
        child: input.child,
    }
}

/// Hash an artifact root edge with the artifact edge domain separator.
pub fn hash_artifact_root_edge(input: &ArtifactRootEdgeInput) -> ArtifactHash {
    let mut bytes = Vec::new();
    push_part(&mut bytes, EDGE_DOMAIN_V1);
    push_part(&mut bytes, ARTIFACT_EDGE_SCHEMA_ID);
    push_part(&mut bytes, ARTIFACT_EDGE_SCHEMA_VERSION);
    push_part(&mut bytes, &input.role);
    push_part(&mut bytes, &input.ordinal.to_string());
    push_root(&mut bytes, &input.parent);
    push_root(&mut bytes, &input.child);
    ArtifactHash::blake3(bytes)
}

fn push_root(bytes: &mut Vec<u8>, root: &ArtifactRootRef) {
    push_part(bytes, root.repo_id.as_str());
    push_part(bytes, root.workspace_id.as_str());
    push_part(bytes, root.scope_id.as_str());
    push_part(bytes, root.generation.as_str());
    push_kind(bytes, &root.root_kind);
    push_hash(bytes, &root.root_hash);
    push_hash(bytes, &root.node_hash);
    push_optional_hash(bytes, root.producer_hash.as_ref());
    push_optional_hash(bytes, root.schema_hash.as_ref());
    push_optional_hash(bytes, root.content_hash.as_ref());
}

fn push_kind(bytes: &mut Vec<u8>, kind: &ArtifactKind) {
    push_part(bytes, kind.as_str());
}

fn push_hash(bytes: &mut Vec<u8>, hash: &ArtifactHash) {
    push_part(bytes, &hash.algorithm);
    push_part(bytes, &hash.value);
}

fn push_optional_hash(bytes: &mut Vec<u8>, hash: Option<&ArtifactHash>) {
    match hash {
        Some(value) => {
            push_part(bytes, "some");
            push_hash(bytes, value);
        }
        None => push_part(bytes, "none"),
    }
}

fn push_part(bytes: &mut Vec<u8>, part: &str) {
    bytes.extend_from_slice(part.len().to_string().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(part.as_bytes());
    bytes.push(0xff);
}
