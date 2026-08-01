use super::publish_provider_source_snapshot_envelope;
use crate::source_index::materialized_current_source_index_snapshot;

#[test]
fn provider_source_envelope_excludes_non_source_generation_anchors() {
    let source_files = std::collections::BTreeMap::from([
        ("Cargo.toml".to_owned(), b"[workspace]\n".to_vec()),
        (
            "src/lib.rs".to_owned(),
            b"pub fn generation_anchor() {}\n".to_vec(),
        ),
    ]);
    let workspace_snapshot = agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(
        source_files.iter().map(|(path, bytes)| {
            (
                path.clone(),
                agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                    bytes,
                )
                .as_str()
                .to_owned(),
            )
        }),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        "provider-digest",
    );
    assert_eq!(source_snapshot.leaf_count, 2);
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        source_files.into_iter().map(|(path, bytes)| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::from(path),
                bytes,
            )
        }),
    );
    let snapshot = materialized_current_source_index_snapshot(
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    )
    .expect("materialize current source-index snapshot");
    let cache_home = std::env::temp_dir().join(format!(
        "asp-provider-source-envelope-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));

    let provider_tree = agent_semantic_content_identity::workspace_merkle_v1::
        WorkspacePathMerkleTreeV1::from_file_digests([(
            "src/lib.rs".to_string(),
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                b"pub fn generation_anchor() {}\n",
            ),
        )])
        .expect("provider workspace Merkle tree");
    let root_hex = provider_tree.root_digest().as_str();
    let mut expected_workspace_root_digest = [0_u8; 32];
    for (index, byte) in expected_workspace_root_digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&root_hex[index * 2..index * 2 + 2], 16)
            .expect("workspace root hex");
    }
    let envelope_path = publish_provider_source_snapshot_envelope(
        crate::source_index::ProviderSourceSnapshotEnvelopePublicationV1 {
            snapshot: &snapshot,
            provider_id: "rs-harness",
            address_provider_digest: &snapshot.source_snapshot.provider_digest,
            source_extensions: &["rs".to_owned()],
            artifact_root: &cache_home,
            provider_workspace_root: &cache_home,
        },
    )
    .expect("provider source envelope");
    let envelope: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&envelope_path).expect("read provider source envelope"),
    )
    .expect("decode provider source envelope");

    assert_eq!(envelope["sourceSnapshot"]["leafCount"], 1);
    assert_eq!(envelope["rootDepth"], 0);
    assert_eq!(envelope["materializationState"], "artifact-complete");
    assert_eq!(envelope["ownerCoverage"], "complete");
    assert_eq!(
        envelope["addressProviderDigest"],
        snapshot.source_snapshot.provider_digest
    );
    assert_eq!(envelope["providerWorkspaceRoot"], ".");
    assert_eq!(
        envelope["providerWorkspaceIdentityDigest"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        envelope["owners"]
            .as_array()
            .expect("provider source owners")
            .len(),
        1
    );
    assert_eq!(envelope["owners"][0]["path"], "src/lib.rs");

    std::fs::remove_dir_all(cache_home).expect("remove provider source envelope fixture");
}
