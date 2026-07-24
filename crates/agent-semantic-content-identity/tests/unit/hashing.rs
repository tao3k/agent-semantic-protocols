use crate::{
    ArtifactLeafInput, DerivedArtifactKeyInput, HASH_ALGORITHM_BLAKE3, hash_blob,
    hash_derived_artifact_key, hash_leaf,
};

#[test]
fn blob_hash_is_stable_blake3_identity() {
    let first = hash_blob(b"struct LiveOnly;");
    let second = hash_blob(b"struct LiveOnly;");

    assert_eq!(first, second);
    assert_eq!(first.algorithm, HASH_ALGORITHM_BLAKE3);
    assert_eq!(first.value.len(), 64);
}

#[test]
fn blob_hash_is_domain_separated_from_artifact_leaf() {
    let payload = b"struct LiveOnly;";
    let blob = hash_blob(payload);
    let artifact_leaf = hash_leaf(ArtifactLeafInput {
        codec: "raw",
        media_type: "application/octet-stream",
        payload,
    });

    assert_ne!(blob, artifact_leaf);
}

#[test]
fn derived_artifact_key_binds_snapshot_provider_and_parameters() {
    let first = hash_derived_artifact_key(DerivedArtifactKeyInput {
        artifact_kind: "source-index",
        schema_id: "asp.source-index.v2",
        snapshot_root: "root-a",
        provider_digest: "provider-a",
        parameters: &[("language", "rust"), ("mode", "items")],
    });
    let reordered = hash_derived_artifact_key(DerivedArtifactKeyInput {
        artifact_kind: "source-index",
        schema_id: "asp.source-index.v2",
        snapshot_root: "root-a",
        provider_digest: "provider-a",
        parameters: &[("mode", "items"), ("language", "rust")],
    });
    let different_root = hash_derived_artifact_key(DerivedArtifactKeyInput {
        artifact_kind: "source-index",
        schema_id: "asp.source-index.v2",
        snapshot_root: "root-b",
        provider_digest: "provider-a",
        parameters: &[("language", "rust"), ("mode", "items")],
    });

    assert_eq!(first, reordered);
    assert_ne!(first, different_root);
}
