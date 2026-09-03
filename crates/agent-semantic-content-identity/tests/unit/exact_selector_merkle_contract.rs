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
    serde_json::from_value(serde_json::json!({
        "canonicalItemSelector": agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentity::new("rust", "function", "run"),
            "rust://crates/example/src/lib.rs#item/function/run",
        ),
        "schemaId": EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID,
        "schemaVersion": EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION,
        "digestAlgorithm": EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM,
        "languageId": "rust",
        "workspaceRootDigest": tree.root_digest(),
        "ownerPath": owner_path,
        "ownerSubtreeDigest": tree
            .owner_subtree_digest(&owner_path)
            .expect("owner leaf"),
        "ownerInclusionProof": tree.inclusion_proof(&owner_path).expect("owner proof"),
        "sourceBlobDigest": source_blob_digest,
        "parserIdentityDigest": digest('e'),
        "queryPackDigest": digest('f'),
        "parserFactDigest": digest('0'),
        "structuralSelector": "rust://crates/example/src/lib.rs#item/function/run",
        "projectionMode": ExactProjectionModeV1::Code,
        "projectionDigest": digest('1'),
    }))
    .expect("valid exact-selector Merkle proof packet")
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
    let mut encoded = serde_json::to_value(&proof).expect("serializable proof");
    encoded["ownerPath"] = serde_json::json!("../outside.rs");
    proof = serde_json::from_value(encoded).expect("deserializable proof");
    assert_eq!(
        proof.validate_shape(),
        Err(ExactSelectorMerkleProofError::OwnerPath)
    );
}

#[test]
fn parser_fact_and_projection_digests_are_domain_separated_and_recomputable() {
    let language_id =
        agent_semantic_content_identity::exact_selector_merkle::ParserLanguageIdV1::from("rust");
    let parser_fact = derive_parser_fact_digest_v1(ParserFactDigestInputV1 {
        language_id: &language_id,
        parser_identity_digest: &digest('e'),
        query_pack_digest: &digest('f'),
        source_blob_digest: &digest('d'),
        normalized_parser_facts: b"normalized-parser-facts",
    });
    let canonical_item_selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentity::new(
                "rust", "function", "run",
            ),
            "rust://crates/example/src/lib.rs#item/function/run",
        );
    let projection = derive_projection_digest_v1(ProjectionDigestInputV1 {
        canonical_item_selector: &canonical_item_selector,
        structural_selector: "rust://crates/example/src/lib.rs#item/function/run",
        projection_mode: ExactProjectionModeV1::Code,
        parser_fact_digest: &parser_fact,
        projection_payload: b"fn run() {}",
    });
    assert_ne!(parser_fact, projection);

    let mut proof = proof();
    let mut encoded = serde_json::to_value(&proof).expect("serializable proof");
    encoded["parserFactDigest"] = serde_json::json!(parser_fact);
    encoded["projectionDigest"] = serde_json::json!(projection);
    proof = serde_json::from_value(encoded).expect("deserializable proof");
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
    ParserFactDigestInputV1, ProjectionDigestInputV1, derive_parser_fact_digest_v1,
    derive_projection_digest_v1, verify_projection_digest_v1,
};
