use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, ExactProjectionModeV1,
};
use agent_semantic_content_identity::exact_selector_projection_packet::{
    ExactSelectorProjectionPacketV1, ExactSelectorProjectionPacketV1Error,
    derive_parser_identity_digest_v1, derive_query_pack_identity_digest_v1,
};
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

fn packet() -> ExactSelectorProjectionPacketV1 {
    let language_id = "rust".to_owned().into();
    let provider_id = "asp-rust-harness".to_owned().into();
    let owner_path = "crates/example/src/lib.rs".to_owned().into();
    let structural_selector = "rust://crates/example/src/lib.rs#item/function/run"
        .to_owned()
        .into();
    agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1(
        agent_semantic_content_identity::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
            source_byte_start: 0,
            source_byte_end: 16,
            language_id: &language_id,
            provider_id: &provider_id,
            canonical_item_selector: agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::new(
                agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentity::new("rust", "function", "run"),
                "rust://crates/example/src/lib.rs#item/function/run",
            ),
            parser_identity_digest: &digest('a'),
            query_pack_digest: &digest('b'),
            owner_path: &owner_path,
            structural_selector: &structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            source: b"fn run() {}",
            normalized_parser_facts: b"normalized-parser-facts",
            projection: b"fn run() {}",
        },
    )
}

#[test]
fn packet_v1_round_trips_and_validates() {
    let packet = packet();
    packet.validate_shape().expect("valid packet");
    let json = serde_json::to_string(&packet).expect("encode packet");
    let decoded: ExactSelectorProjectionPacketV1 =
        serde_json::from_str(&json).expect("decode packet");
    assert_eq!(decoded, packet);
}

#[test]
fn packet_v1_rejects_noncanonical_payload_and_owner_escape() {
    let mut invalid_payload = packet();
    invalid_payload.projection_payload_base64 = "not base64".to_owned().into();
    assert_eq!(
        invalid_payload.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::ProjectionPayload)
    );

    let mut noncanonical_payload = packet();
    noncanonical_payload.projection_payload_base64 = "Zh==".to_owned().into();
    assert_eq!(
        noncanonical_payload.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::ProjectionPayload)
    );

    let mut invalid_owner = packet();
    invalid_owner.owner_path = "../outside.rs".to_owned().into();
    assert_eq!(
        invalid_owner.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::OwnerPath)
    );
}

#[test]
fn packet_v1_enrichment_binds_current_workspace_membership() {
    let packet = packet();
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (
            packet.owner_path().as_str().to_owned(),
            packet.source_blob_digest.clone(),
        ),
        ("crates/other/src/lib.rs".to_owned(), digest('e')),
    ])
    .expect("workspace tree");
    let record = packet
        .enrich_projection_record(&tree)
        .expect("enriched record");
    assert_eq!(record.projection_payload, b"fn run() {}");
    let key = ExactSelectorMerkleLookupKeyV1 {
        language_id: record.proof.language_id(),
        workspace_root_digest: record.proof.workspace_root_digest(),
        owner_path: record.proof.owner_path(),
        owner_subtree_digest: record.proof.owner_subtree_digest(),
        source_blob_digest: record.proof.source_blob_digest(),
        parser_identity_digest: record.proof.parser_identity_digest(),
        query_pack_digest: record.proof.query_pack_digest(),
        structural_selector: record.proof.structural_selector(),
        projection_mode: record.proof.projection_mode().clone(),
    };
    record.validate_warm_hit(&key).expect("validated record");
}

#[test]
fn packet_v1_reuses_owner_proof_without_changing_the_record() {
    let packet = packet();
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (
            packet.owner_path().as_str().to_owned(),
            packet.source_blob_digest.clone(),
        ),
        ("crates/other/src/lib.rs".to_owned(), digest('e')),
    ])
    .expect("workspace tree");
    let owner_proof = tree
        .inclusion_proof(packet.owner_path().as_str())
        .expect("owner proof");

    let direct = packet
        .clone()
        .enrich_projection_record(&tree)
        .expect("direct enrichment");
    let reused = packet
        .enrich_projection_record_with_owner_inclusion_proof(&tree, &owner_proof)
        .expect("owner-proof enrichment");

    assert_eq!(reused, direct);
}

#[test]
fn activation_identity_digests_bind_content_not_labels() {
    let parser = derive_parser_identity_digest_v1(
        &"rs-harness".to_owned().into(),
        &"exec-a".to_owned().into(),
        &"registry-a".to_owned().into(),
    );
    assert_eq!(
        parser,
        derive_parser_identity_digest_v1(
            &"rs-harness".to_owned().into(),
            &"exec-a".to_owned().into(),
            &"registry-a".to_owned().into(),
        )
    );
    assert_ne!(
        parser,
        derive_parser_identity_digest_v1(
            &"rs-harness".to_owned().into(),
            &"exec-b".to_owned().into(),
            &"registry-a".to_owned().into(),
        )
    );

    let query_pack = derive_query_pack_identity_digest_v1(br#"{"descriptorId":"rust-v1"}"#);
    assert_ne!(
        query_pack,
        derive_query_pack_identity_digest_v1(br#"{"descriptorId":"rust-v1","recipes":[]}"#)
    );
}
