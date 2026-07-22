use agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1;
use agent_semantic_content_identity::workspace_merkle_v1::{
    WorkspaceMerkleV1Error, WorkspacePathMerkleTreeV1, verify_owner_inclusion_v1,
};

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

fn entries() -> Vec<(String, ContentDigestV1)> {
    vec![
        ("crates/a/src/lib.rs".to_owned(), digest('a')),
        ("crates/b/src/lib.rs".to_owned(), digest('b')),
        ("crates/c/src/lib.rs".to_owned(), digest('c')),
    ]
}

#[test]
fn root_is_order_independent_and_every_leaf_has_a_valid_proof() {
    let forward = WorkspacePathMerkleTreeV1::from_file_digests(entries()).expect("tree");
    let mut reversed_entries = entries();
    reversed_entries.reverse();
    let reversed = WorkspacePathMerkleTreeV1::from_file_digests(reversed_entries).expect("tree");
    assert_eq!(forward.root_digest(), reversed.root_digest());

    for (path, source_digest) in entries() {
        assert!(verify_owner_inclusion_v1(
            &path,
            &source_digest,
            forward.owner_subtree_digest(&path).expect("owner leaf"),
            &forward.inclusion_proof(&path).expect("owner proof"),
            forward.root_digest(),
        ));
    }
}

#[test]
fn changed_source_or_root_fails_closed() {
    let tree = WorkspacePathMerkleTreeV1::from_file_digests(entries()).expect("tree");
    let path = "crates/a/src/lib.rs";
    let proof = tree.inclusion_proof(path).expect("proof");
    let owner_digest = tree.owner_subtree_digest(path).expect("owner leaf");
    assert!(!verify_owner_inclusion_v1(
        path,
        &digest('9'),
        owner_digest,
        &proof,
        tree.root_digest(),
    ));
    assert!(!verify_owner_inclusion_v1(
        path,
        &digest('a'),
        owner_digest,
        &proof,
        &digest('9'),
    ));
}

#[test]
fn invalid_and_duplicate_paths_are_rejected() {
    assert_eq!(
        WorkspacePathMerkleTreeV1::from_file_digests([("../outside".to_owned(), digest('a'))]),
        Err(WorkspaceMerkleV1Error::InvalidPath)
    );
    assert_eq!(
        WorkspacePathMerkleTreeV1::from_file_digests([
            ("crates/a/src/lib.rs".to_owned(), digest('a')),
            ("crates/a/src/lib.rs".to_owned(), digest('b')),
        ]),
        Err(WorkspaceMerkleV1Error::DuplicatePath)
    );
}
