// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleV1Error;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspaceOwnerInclusionV1;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;
use agent_semantic_content_identity::workspace_merkle_v1::verify_owner_inclusion_v1;

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
        assert!(verify_owner_inclusion_v1(WorkspaceOwnerInclusionV1 {
            owner_path: &path,
            source_blob_digest: &source_digest,
            expected_owner_subtree_digest: forward.owner_subtree_digest(&path).expect("owner leaf"),
            inclusion_proof: &forward.inclusion_proof(&path).expect("owner proof"),
            expected_workspace_root_digest: forward.root_digest(),
        }));
    }
}

#[test]
fn changed_source_or_root_fails_closed() {
    let tree = WorkspacePathMerkleTreeV1::from_file_digests(entries()).expect("tree");
    let path = "crates/a/src/lib.rs";
    let proof = tree.inclusion_proof(path).expect("proof");
    let owner_digest = tree.owner_subtree_digest(path).expect("owner leaf");
    assert!(!verify_owner_inclusion_v1(WorkspaceOwnerInclusionV1 {
        owner_path: path,
        source_blob_digest: &digest('9'),
        expected_owner_subtree_digest: owner_digest,
        inclusion_proof: &proof,
        expected_workspace_root_digest: tree.root_digest(),
    }));
    assert!(!verify_owner_inclusion_v1(WorkspaceOwnerInclusionV1 {
        owner_path: path,
        source_blob_digest: &digest('a'),
        expected_owner_subtree_digest: owner_digest,
        inclusion_proof: &proof,
        expected_workspace_root_digest: &digest('9'),
    }));
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

#[test]
fn all_owner_proofs_reuse_precomputed_levels_within_wall_budget() {
    let entries = (0..256)
        .map(|index| {
            (
                format!("crates/member-{index:03}/src/lib.rs"),
                ContentDigestV1::parse(format!("{index:064x}")).expect("valid digest"),
            )
        })
        .collect::<Vec<_>>();
    let tree = WorkspacePathMerkleTreeV1::from_file_digests(entries.clone()).expect("tree");
    let started = std::time::Instant::now();

    for (path, source_digest) in entries {
        let proof = tree.inclusion_proof(&path).expect("owner proof");
        assert!(verify_owner_inclusion_v1(WorkspaceOwnerInclusionV1 {
            owner_path: &path,
            source_blob_digest: &source_digest,
            expected_owner_subtree_digest: tree.owner_subtree_digest(&path).expect("owner leaf"),
            inclusion_proof: &proof,
            expected_workspace_root_digest: tree.root_digest(),
        }));
    }

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "cached owner proofs must not rebuild the complete tree per owner: {:?}",
        started.elapsed(),
    );
}
