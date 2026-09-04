use super::ContentDigestV1;
use super::WorkspaceMerkleDeltaOperationIncrementalV1;
use super::WorkspaceMerkleIncrementalV1Error;
use super::WorkspacePathMerkleTreeIncrementalV1;
use super::verify_owner_inclusion_incremental_v1;
use crate::exact_selector_merkle::parse_content_digest_v1;

fn digest(byte: char) -> ContentDigestV1 {
    parse_content_digest_v1(&byte.to_string().repeat(64)).expect("test digest")
}

#[test]
fn one_owner_update_rewrites_only_the_owner_path() {
    let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
        ("src/lib.rs".to_owned(), digest('1')),
        ("src/sibling.rs".to_owned(), digest('2')),
    ])
    .expect("base tree");
    let previous_node_count = tree.node_count();
    let (successor, metrics) = tree
        .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
            owner_path: "src/lib.rs".to_owned(),
            previous_source_blob_digest: Some(digest('1')),
            source_blob_digest: digest('3'),
        }])
        .expect("incremental update");
    assert_ne!(successor.root_digest(), tree.root_digest());
    assert_eq!(successor.leaf_count(), 2);
    assert_eq!(successor.node_count(), previous_node_count);
    assert_eq!(metrics.touched_leaf_count, 1);
    assert_eq!(metrics.written_node_count, "src/lib.rs".len() + 1);
    assert!(metrics.reused_node_count > 0);
    assert_eq!(metrics.full_merkle_rebuilds, 0);
}

#[test]
fn inclusion_proof_round_trips_after_incremental_update() {
    let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
        ("src/lib.rs".to_owned(), digest('1')),
        ("src/sibling.rs".to_owned(), digest('2')),
    ])
    .expect("base tree");
    let (successor, _) = tree
        .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
            owner_path: "src/lib.rs".to_owned(),
            previous_source_blob_digest: Some(digest('1')),
            source_blob_digest: digest('3'),
        }])
        .expect("incremental update");
    let proof = successor
        .inclusion_proof("src/lib.rs")
        .expect("owner proof");
    assert!(
        verify_owner_inclusion_incremental_v1(&proof, successor.root_digest())
            .expect("valid proof")
    );
    assert!(
        !verify_owner_inclusion_incremental_v1(&proof, tree.root_digest()).expect("stale root")
    );
}

#[test]
fn delta_preconditions_fail_closed() {
    let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([(
        "src/lib.rs".to_owned(),
        digest('1'),
    )])
    .expect("base tree");
    let error = tree
        .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Remove {
            owner_path: "src/lib.rs".to_owned(),
            previous_source_blob_digest: digest('2'),
        }])
        .expect_err("digest drift must fail");
    assert_eq!(
        error,
        WorkspaceMerkleIncrementalV1Error::PreviousDigestMismatch
    );
}

#[test]
fn node_table_is_deterministic_and_linear_in_tree_nodes() {
    let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
        ("src/lib.rs".to_owned(), digest('1')),
        ("src/sibling.rs".to_owned(), digest('2')),
    ])
    .expect("tree");
    let records = tree.node_table();
    assert_eq!(records.len(), tree.node_count());
    assert_eq!(records.first().expect("root record").path_prefix_hex, "");
    assert_eq!(
        records.first().expect("root record").digest,
        *tree.root_digest()
    );
    assert_eq!(tree.node_table_digest(), tree.node_table_digest());

    let (successor, _) = tree
        .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
            owner_path: "src/lib.rs".to_owned(),
            previous_source_blob_digest: Some(digest('1')),
            source_blob_digest: digest('3'),
        }])
        .expect("successor");
    assert_ne!(tree.node_table_digest(), successor.node_table_digest());
}
