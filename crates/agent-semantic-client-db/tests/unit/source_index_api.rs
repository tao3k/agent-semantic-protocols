#[test]
fn materialized_snapshot_binds_root_leaf_and_owner_counts() {
    let owner = "src/lib.rs";
    let bytes = b"pub fn exact_owner() {}".to_vec();
    let owner_digest = blake3::hash(&bytes).to_hex().to_string();
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner, owner_digest.as_str())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "provider-digest".to_string(),
    );
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::new(owner),
            bytes,
        )]);
    let materialized = super::materialized_current_source_index_snapshot(
        workspace_snapshot,
        source_snapshot,
        source_blobs,
    )
    .expect("materialize current source-index snapshot");
    let generation = materialized.workspace_generation.evidence();
    assert_eq!(
        generation.root_digest,
        materialized.source_snapshot.root_digest
    );
    assert_eq!(
        generation.leaf_count,
        materialized.source_snapshot.leaf_count as u64
    );
    assert_eq!(
        generation.owner_count,
        materialized.source_blobs.len() as u64
    );
}
