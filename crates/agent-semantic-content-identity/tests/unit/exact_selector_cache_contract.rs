use agent_semantic_content_identity::exact_selector_cache::{
    ExactSelectorMerkleLookupKeyV1, ExactSelectorMerkleMissV1, ExactSelectorProjectionRecordV1,
    ExactSelectorWarmSideEffectsV1,
};
use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM, EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID,
    EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION, ExactProjectionModeV1, ExactSelectorMerkleProofV1,
    derive_parser_fact_digest_v1, derive_projection_digest_v1,
};
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

pub(crate) fn record() -> ExactSelectorProjectionRecordV1 {
    let projection_payload = b"fn run() {}".to_vec();
    let owner_path = "crates/example/src/lib.rs".to_owned();
    let source_blob_digest = digest('d');
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (owner_path.clone(), source_blob_digest.clone()),
        ("crates/other/src/lib.rs".to_owned(), digest('2')),
    ])
    .expect("valid Merkle tree");
    let parser_fact_digest = derive_parser_fact_digest_v1(
        "rust",
        &digest('e'),
        &digest('f'),
        &digest('d'),
        b"normalized-parser-facts",
    );
    let structural_selector = "rust://crates/example/src/lib.rs#item/function/run".to_owned();
    let projection_digest = derive_projection_digest_v1(
        &agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new(
                "rust", "function", "run",
            ),
            structural_selector.clone(),
        ),
        &structural_selector,
        ExactProjectionModeV1::Code,
        &parser_fact_digest,
        &projection_payload,
    );
    ExactSelectorProjectionRecordV1 {
        proof: ExactSelectorMerkleProofV1 {
            canonical_item_selector: agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new(
                    "rust", "function", "run",
                ),
                structural_selector.clone(),
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
            parser_fact_digest,
            structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            projection_digest,
        },
        projection_payload,
    }
}

pub(crate) fn key<'a>(
    record: &'a ExactSelectorProjectionRecordV1,
) -> ExactSelectorMerkleLookupKeyV1<'a> {
    ExactSelectorMerkleLookupKeyV1 {
        language_id: &record.proof.language_id,
        workspace_root_digest: &record.proof.workspace_root_digest,
        owner_path: &record.proof.owner_path,
        owner_subtree_digest: &record.proof.owner_subtree_digest,
        source_blob_digest: &record.proof.source_blob_digest,
        parser_identity_digest: &record.proof.parser_identity_digest,
        query_pack_digest: &record.proof.query_pack_digest,
        structural_selector: &record.proof.structural_selector,
        projection_mode: record.proof.projection_mode,
    }
}

#[test]
fn valid_warm_hit_has_zero_side_effects() {
    let record = record();
    let hit = record
        .validate_warm_hit(&key(&record))
        .expect("valid warm hit");
    assert_eq!(hit.projection_payload, b"fn run() {}");
    assert_eq!(hit.side_effects, ExactSelectorWarmSideEffectsV1::ZERO);
}

#[test]
fn failed_owner_inclusion_is_a_typed_miss() {
    let record = record();
    let mut invalid = record.clone();
    invalid.proof.workspace_root_digest = digest('9');
    let invalid_key = key(&invalid);
    assert_eq!(
        invalid.validate_warm_hit(&invalid_key),
        Err(ExactSelectorMerkleMissV1::InvalidProofShape)
    );
}

#[test]
fn changed_projection_payload_is_a_typed_miss() {
    let mut record = record();
    record.projection_payload = b"fn changed() {}".to_vec();
    assert_eq!(
        record.validate_warm_hit(&key(&record)),
        Err(ExactSelectorMerkleMissV1::ProjectionDigestMismatch)
    );
}

#[test]
fn validated_projection_reuses_proof_but_rebinds_every_lookup_key() {
    let record = record();
    let lookup_key = key(&record);
    let validated = agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record.clone(),
        &lookup_key,
    )
    .expect("valid hydration");
    let hit = validated
        .validate_warm_hit(&lookup_key)
        .expect("validated warm hit");
    assert_eq!(hit.side_effects, ExactSelectorWarmSideEffectsV1::ZERO);

    let mut different_identity = record;
    different_identity.proof.query_pack_digest = digest('8');
    assert_eq!(
        validated.validate_warm_hit(&key(&different_identity)),
        Err(ExactSelectorMerkleMissV1::IdentityMismatch)
    );
}
