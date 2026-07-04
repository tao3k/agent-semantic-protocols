//! Content-addressed artifact identity primitives for ASP.
//!
//! This crate owns the Merkle artifact forest hash contract. It intentionally
//! does not own DB writes, filesystem storage, or search routing.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

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

/// Stable content digest used by ASP artifact roots, nodes, and leaves.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactHash {
    /// Digest algorithm token, currently always `blake3`.
    pub algorithm: String,
    /// Lowercase hex digest value.
    pub value: String,
}

/// Named artifact JSON boundary for canonical JSON hashing.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ArtifactJson {
    value: serde_json::Value,
}

impl ArtifactJson {
    /// Build an artifact JSON boundary from a dynamic JSON value.
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    fn as_value(&self) -> &serde_json::Value {
        &self.value
    }
}

/// Stable State Core repository identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactRepoId(String);

/// Stable State Core workspace identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactWorkspaceId(String);

/// Stable artifact scope identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactScopeId(String);

/// Stable artifact generation identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactGeneration(String);

/// Artifact node or root kind.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct ArtifactKind(String);

impl ArtifactRepoId {
    /// Create a repository identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the repository identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactWorkspaceId {
    /// Create a workspace identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the workspace identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactScopeId {
    /// Create an artifact scope identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the scope identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactGeneration {
    /// Create an artifact generation identity.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the generation identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactKind {
    /// Create an artifact kind token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the artifact kind token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactHash {
    /// Hash bytes with the canonical artifact digest algorithm.
    pub fn blake3(bytes: impl AsRef<[u8]>) -> Self {
        Self::from_blake3_output(blake3::hash(bytes.as_ref()))
    }

    fn from_blake3_output(output: blake3::Hash) -> Self {
        Self {
            algorithm: HASH_ALGORITHM_BLAKE3.to_string(),
            value: output.to_hex().to_string(),
        }
    }

    /// Render the hash as an integrity reference such as `blake3:<hex>`.
    pub fn as_integrity_ref(&self) -> String {
        format!("{}:{}", self.algorithm, self.value)
    }
}

impl fmt::Display for ArtifactHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_integrity_ref())
    }
}

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

impl ArtifactRootRef {
    /// Build a root reference from root input and optional provenance hashes.
    pub fn from_input(
        input: ArtifactRootInput,
        producer_hash: Option<ArtifactHash>,
        schema_hash: Option<ArtifactHash>,
        content_hash: Option<ArtifactHash>,
    ) -> Self {
        let root_hash = hash_root(&input);
        Self {
            repo_id: input.repo_id,
            workspace_id: input.workspace_id,
            scope_id: input.scope_id,
            generation: input.generation,
            root_kind: input.root_kind,
            root_hash,
            node_hash: input.node_hash,
            producer_hash,
            schema_hash,
            content_hash,
        }
    }
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

/// Hash a raw artifact leaf with codec and media-type domain separation.
pub fn hash_leaf(input: ArtifactLeafInput<'_>) -> ArtifactHash {
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, LEAF_DOMAIN_V1.as_bytes());
    update_part(&mut hasher, input.codec.as_bytes());
    update_part(&mut hasher, input.media_type.as_bytes());
    update_part(&mut hasher, input.payload);
    ArtifactHash::from_blake3_output(hasher.finalize())
}

/// Hash JSON after canonical object-key normalization.
pub fn hash_normalized_json(value: &ArtifactJson) -> ArtifactHash {
    let mut normalized = Vec::new();
    write_canonical_json(value.as_value(), &mut normalized);
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, JSON_DOMAIN_V1.as_bytes());
    update_part(&mut hasher, &normalized);
    ArtifactHash::from_blake3_output(hasher.finalize())
}

/// Hash a Merkle artifact node with deterministic child-edge ordering.
pub fn hash_node(input: &ArtifactNodeInput) -> ArtifactHash {
    let mut children = input.children.clone();
    children.sort_by(compare_child_refs);
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, NODE_DOMAIN_V1.as_bytes());
    update_part(&mut hasher, input.kind.as_str().as_bytes());
    update_part(&mut hasher, input.schema_id.as_bytes());
    update_part(&mut hasher, input.schema_version.as_bytes());
    update_optional_hash(&mut hasher, input.producer_hash.as_ref());
    update_optional_hash(&mut hasher, input.payload_hash.as_ref());
    update_optional_hash(&mut hasher, input.metadata_hash.as_ref());
    for child in children {
        update_part(&mut hasher, child.role.as_bytes());
        update_part(&mut hasher, child.name.as_bytes());
        update_part(&mut hasher, child.child_hash.algorithm.as_bytes());
        update_part(&mut hasher, child.child_hash.value.as_bytes());
        update_part(&mut hasher, &child.ordinal.to_be_bytes());
    }
    ArtifactHash::from_blake3_output(hasher.finalize())
}

/// Hash a State Core scoped artifact root.
pub fn hash_root(input: &ArtifactRootInput) -> ArtifactHash {
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, ROOT_DOMAIN_V1.as_bytes());
    update_part(&mut hasher, input.repo_id.as_str().as_bytes());
    update_part(&mut hasher, input.workspace_id.as_str().as_bytes());
    update_part(&mut hasher, input.scope_id.as_str().as_bytes());
    update_part(&mut hasher, input.generation.as_str().as_bytes());
    update_part(&mut hasher, input.root_kind.as_str().as_bytes());
    update_part(&mut hasher, input.node_hash.algorithm.as_bytes());
    update_part(&mut hasher, input.node_hash.value.as_bytes());
    ArtifactHash::from_blake3_output(hasher.finalize())
}

fn compare_child_refs(left: &ArtifactChildRef, right: &ArtifactChildRef) -> Ordering {
    left.role
        .cmp(&right.role)
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.child_hash.algorithm.cmp(&right.child_hash.algorithm))
        .then_with(|| left.child_hash.value.cmp(&right.child_hash.value))
        .then_with(|| left.ordinal.cmp(&right.ordinal))
}

fn update_optional_hash(hasher: &mut blake3::Hasher, value: Option<&ArtifactHash>) {
    match value {
        Some(value) => {
            update_part(hasher, b"some");
            update_part(hasher, value.algorithm.as_bytes());
            update_part(hasher, value.value.as_bytes());
        }
        None => update_part(hasher, b"none"),
    }
}

fn update_part(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn write_canonical_json(value: &serde_json::Value, output: &mut Vec<u8>) {
    match value {
        serde_json::Value::Null => output.extend_from_slice(b"null"),
        serde_json::Value::Bool(true) => output.extend_from_slice(b"true"),
        serde_json::Value::Bool(false) => output.extend_from_slice(b"false"),
        serde_json::Value::Number(number) => {
            output.extend_from_slice(number.to_string().as_bytes())
        }
        serde_json::Value::String(text) => {
            let encoded =
                serde_json::to_string(text).expect("JSON string serialization cannot fail");
            output.extend_from_slice(encoded.as_bytes());
        }
        serde_json::Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_json(value, output);
            }
            output.push(b']');
        }
        serde_json::Value::Object(map) => {
            output.push(b'{');
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                let encoded_key =
                    serde_json::to_string(key).expect("JSON key serialization cannot fail");
                output.extend_from_slice(encoded_key.as_bytes());
                output.push(b':');
                write_canonical_json(value, output);
            }
            output.push(b'}');
        }
    }
}
