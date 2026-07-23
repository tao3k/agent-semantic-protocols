use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM, EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID,
    EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION, ExactProjectionModeV1,
    ExactSelectorMerkleProofError, ExactSelectorMerkleProofV1,
};
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

fn proof() -> ExactSelectorMerkleProofV1 {
    let owner_path = "crates/example/src/lib.rs".to_owned();
    let source_blob_digest = digest('d');
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (owner_path.clone(), source_blob_digest.clone()),
        ("crates/other/src/lib.rs".to_owned(), digest('2')),
    ])
    .expect("valid Merkle tree");
    ExactSelectorMerkleProofV1 {
        canonical_item_selector: agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new("rust", "function", "run"),
            "rust://crates/example/src/lib.rs#item/function/run",
        ),
        schema_id: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID.to_owned(),
        schema_version: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION.to_owned(),
        digest_algorithm: EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM.to_owned(),
        language_id: "rust".to_owned(),
        workspace_root_digest: tree.root_digest().clone(),
        owner_path: owner_path.clone(),
        owner_subtree_digest: tree
            .owner_subtree_digest(&owner_path)
            .expect("owner leaf")
            .clone(),
        owner_inclusion_proof: tree.inclusion_proof(&owner_path).expect("owner proof"),
        source_blob_digest,
        parser_identity_digest: digest('e'),
        query_pack_digest: digest('f'),
        parser_fact_digest: digest('0'),
        structural_selector: "rust://crates/example/src/lib.rs#item/function/run".to_owned(),
        projection_mode: ExactProjectionModeV1::Code,
        projection_digest: digest('1'),
    }
}

#[test]
fn valid_v1_proof_shape_round_trips() {
    let proof = proof();
    proof.validate_shape().expect("valid proof shape");
    let encoded = serde_json::to_value(&proof).expect("serialize proof");
    assert_eq!(encoded["schemaVersion"], "1");
    assert_eq!(encoded["digestAlgorithm"], "blake3-256");
    assert_eq!(encoded["ownerInclusionProof"][0]["side"], "right");
    let decoded: ExactSelectorMerkleProofV1 =
        serde_json::from_value(encoded).expect("deserialize proof");
    assert_eq!(decoded, proof);
}

#[test]
fn digest_parser_rejects_non_canonical_values() {
    assert_eq!(
        ContentDigestV1::parse("A".repeat(64)),
        Err(ExactSelectorMerkleProofError::ContentDigest)
    );
    assert_eq!(
        ContentDigestV1::parse("a".repeat(63)),
        Err(ExactSelectorMerkleProofError::ContentDigest)
    );
}

#[test]
fn proof_rejects_parent_directory_owner_path() {
    let mut proof = proof();
    proof.owner_path = "../outside.rs".to_owned();
    assert_eq!(
        proof.validate_shape(),
        Err(ExactSelectorMerkleProofError::OwnerPath)
    );
}

#[test]
fn parser_fact_and_projection_digests_are_domain_separated_and_recomputable() {
    let parser_fact = derive_parser_fact_digest_v1(
        "rust",
        &digest('e'),
        &digest('f'),
        &digest('d'),
        b"normalized-parser-facts",
    );
    let projection = derive_projection_digest_v1(
        &agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new(
                "rust", "function", "run",
            ),
            "rust://crates/example/src/lib.rs#item/function/run",
        ),
        "rust://crates/example/src/lib.rs#item/function/run",
        ExactProjectionModeV1::Code,
        &parser_fact,
        b"fn run() {}",
    );
    assert_ne!(parser_fact, projection);

    let mut proof = proof();
    proof.parser_fact_digest = parser_fact;
    proof.projection_digest = projection;
    assert_eq!(
        verify_projection_digest_v1(&proof, b"fn run() {}"),
        Ok(true)
    );
    assert_eq!(
        verify_projection_digest_v1(&proof, b"fn changed() {}"),
        Ok(false)
    );
}
use agent_semantic_content_identity::exact_selector_merkle::{
    derive_parser_fact_digest_v1, derive_projection_digest_v1, verify_projection_digest_v1,
};
