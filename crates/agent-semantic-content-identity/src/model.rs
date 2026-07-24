//! Serializable artifact inputs, child and root references, and identity documents.

use serde::{Deserialize, Serialize};

use crate::domain::{
    ARTIFACT_IDENTITY_SCHEMA_ID, ARTIFACT_IDENTITY_SCHEMA_VERSION, HASH_ALGORITHM_BLAKE3,
};
use crate::value::{
    ArtifactGeneration, ArtifactHash, ArtifactKind, ArtifactRepoId, ArtifactScopeId,
    ArtifactWorkspaceId,
};

/// Raw payload leaf hash input.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLeafInput<'a> {
    /// Payload codec, for example `json`, `text`, or `bytes`.
    pub codec: &'a str,
    /// Payload media type, for example `application/json`.
    pub media_type: &'a str,
    #[serde(skip)]
    /// Payload bytes hashed with the leaf domain separator.
    pub payload: &'a [u8],
}

/// Stable child edge included in an artifact node hash.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactChildRef {
    /// Semantic edge role such as `source`, `providerOutput`, or `metadata`.
    pub role: String,
    /// Stable edge-local name.
    pub name: String,
    /// Child node or leaf hash.
    pub child_hash: ArtifactHash,
    /// Stable ordinal used when a role/name pair has ordered children.
    pub ordinal: u64,
}

impl ArtifactChildRef {
    /// Build a child edge reference.
    pub fn new(
        role: impl Into<String>,
        name: impl Into<String>,
        child_hash: ArtifactHash,
        ordinal: u64,
    ) -> Self {
        Self {
            role: role.into(),
            name: name.into(),
            child_hash,
            ordinal,
        }
    }
}

/// Input for a Merkle artifact node hash.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactNodeInput {
    /// Artifact node kind such as `sourceSnapshot` or `compactGraph`.
    pub kind: ArtifactKind,
    /// Schema identifier for the node payload contract.
    pub schema_id: String,
    /// Schema version for the node payload contract.
    pub schema_version: String,
    /// Optional producer identity hash.
    pub producer_hash: Option<ArtifactHash>,
    /// Optional payload leaf hash.
    pub payload_hash: Option<ArtifactHash>,
    /// Optional metadata hash.
    pub metadata_hash: Option<ArtifactHash>,
    /// Child edges sorted deterministically before hashing.
    pub children: Vec<ArtifactChildRef>,
}

/// Input for a State Core scoped artifact root hash.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRootInput {
    /// Stable State Core repository identity.
    pub repo_id: ArtifactRepoId,
    /// Stable State Core workspace identity.
    pub workspace_id: ArtifactWorkspaceId,
    /// Stable scope identity, usually `default` in Phase 1.
    pub scope_id: ArtifactScopeId,
    /// Artifact generation identity.
    pub generation: ArtifactGeneration,
    /// Root kind such as `sourceSnapshot` or `dynamicOverlay`.
    pub root_kind: ArtifactKind,
    /// Hash of the root node.
    pub node_hash: ArtifactHash,
}

/// Compact Merkle root reference serialized into receipts and manifests.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRootRef {
    /// Stable State Core repository identity.
    pub repo_id: ArtifactRepoId,
    /// Stable State Core workspace identity.
    pub workspace_id: ArtifactWorkspaceId,
    /// Stable scope identity, usually `default` in Phase 1.
    pub scope_id: ArtifactScopeId,
    /// Artifact generation identity.
    pub generation: ArtifactGeneration,
    /// Root kind such as `sourceSnapshot` or `dynamicOverlay`.
    pub root_kind: ArtifactKind,
    /// Hash of this root.
    pub root_hash: ArtifactHash,
    /// Hash of the root node.
    pub node_hash: ArtifactHash,
    /// Optional producer identity hash.
    pub producer_hash: Option<ArtifactHash>,
    /// Optional schema identity hash.
    pub schema_hash: Option<ArtifactHash>,
    /// Optional content identity hash.
    pub content_hash: Option<ArtifactHash>,
}

/// Artifact identity document matching `semantic-artifact-identity.v1`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactIdentityDocument {
    /// Schema id, always `semantic-artifact-identity`.
    schema_id: String,
    /// Schema version, always `1`.
    schema_version: String,
    /// Hash algorithm token, always `blake3` in v1.
    hash_algorithm: String,
    /// Root references carried by this document.
    roots: Vec<ArtifactRootRef>,
}

impl ArtifactIdentityDocument {
    /// Build an artifact identity document from root references.
    pub fn new(roots: Vec<ArtifactRootRef>) -> Self {
        Self {
            schema_id: ARTIFACT_IDENTITY_SCHEMA_ID.to_string(),
            schema_version: ARTIFACT_IDENTITY_SCHEMA_VERSION.to_string(),
            hash_algorithm: HASH_ALGORITHM_BLAKE3.to_string(),
            roots,
        }
    }
}
