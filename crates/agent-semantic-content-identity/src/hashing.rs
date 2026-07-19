//! Deterministic artifact hashing and canonical JSON encoding operations.

use std::cmp::Ordering;

use crate::domain::{JSON_DOMAIN_V1, LEAF_DOMAIN_V1, NODE_DOMAIN_V1, ROOT_DOMAIN_V1};
use crate::model::{ArtifactChildRef, ArtifactLeafInput, ArtifactNodeInput, ArtifactRootInput};
use crate::value::{ArtifactHash, ArtifactJson};

/// Inputs that make a derived cache artifact reproducible from source truth.
#[derive(Clone, Copy, Debug)]
pub struct DerivedArtifactKeyInput<'a> {
    /// Stable artifact family, such as `source-index` or `compact-graph`.
    pub artifact_kind: &'a str,
    /// Versioned schema that defines the artifact payload.
    pub schema_id: &'a str,
    /// Merkle workspace snapshot root from which the artifact is derived.
    pub snapshot_root: &'a str,
    /// Provider/parser identity digest used to build the artifact.
    pub provider_digest: &'a str,
    /// Deterministic, order-independent key/value build parameters.
    pub parameters: &'a [(&'a str, &'a str)],
}

/// Hash a disposable derived artifact key from its complete source authority.
pub fn hash_derived_artifact_key(input: DerivedArtifactKeyInput<'_>) -> ArtifactHash {
    let mut parameters = input.parameters.to_vec();
    parameters.sort_unstable();
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, b"asp.content-identity.derived-artifact.v1");
    update_part(&mut hasher, input.artifact_kind.as_bytes());
    update_part(&mut hasher, input.schema_id.as_bytes());
    update_part(&mut hasher, input.snapshot_root.as_bytes());
    update_part(&mut hasher, input.provider_digest.as_bytes());
    for (name, value) in parameters {
        update_part(&mut hasher, name.as_bytes());
        update_part(&mut hasher, value.as_bytes());
    }
    ArtifactHash::from_blake3_output(hasher.finalize())
}

/// Hash source/blob bytes with a stable, domain-separated BLAKE3 identity.
pub fn hash_blob(payload: &[u8]) -> ArtifactHash {
    let mut hasher = blake3::Hasher::new();
    update_part(&mut hasher, b"asp.content-identity.blob.v1");
    update_part(&mut hasher, payload);
    ArtifactHash::from_blake3_output(hasher.finalize())
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

impl crate::model::ArtifactRootRef {
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
