use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, ExactProjectionModeV1,
};
use agent_semantic_content_identity::exact_selector_projection_packet::{
    EXACT_SELECTOR_PROJECTION_PACKET_DIGEST_ALGORITHM, EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_ID,
    EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_VERSION, ExactSelectorProjectionEncodingV1,
    ExactSelectorProjectionPacketV1, ExactSelectorProjectionPacketV1Error,
    derive_parser_identity_digest_v1, derive_query_pack_identity_digest_v1,
};
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

fn packet() -> ExactSelectorProjectionPacketV1 {
    ExactSelectorProjectionPacketV1 {
        schema_id: EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_ID.to_owned(),
        schema_version: EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_VERSION.to_owned(),
        digest_algorithm: EXACT_SELECTOR_PROJECTION_PACKET_DIGEST_ALGORITHM.to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust-harness".to_owned(),
        parser_identity_digest: digest('a'),
        query_pack_digest: digest('b'),
        owner_path: "crates/example/src/lib.rs".to_owned(),
        source_blob_digest: digest('c'),
        parser_fact_digest: digest('d'),
        structural_selector: "rust://crates/example/src/lib.rs#item/function/run".to_owned(),
        projection_mode: ExactProjectionModeV1::Code,
        projection_encoding: ExactSelectorProjectionEncodingV1::Base64,
        projection_payload_base64: "Zm4gcnVuKCkge30=".to_owned(),
    }
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
    invalid_payload.projection_payload_base64 = "not base64".to_owned();
    assert_eq!(
        invalid_payload.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::ProjectionPayload)
    );

    let mut noncanonical_payload = packet();
    noncanonical_payload.projection_payload_base64 = "Zh==".to_owned();
    assert_eq!(
        noncanonical_payload.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::ProjectionPayload)
    );

    let mut invalid_owner = packet();
    invalid_owner.owner_path = "../outside.rs".to_owned();
    assert_eq!(
        invalid_owner.validate_shape(),
        Err(ExactSelectorProjectionPacketV1Error::OwnerPath)
    );
}

#[test]
fn packet_v1_enrichment_binds_current_workspace_membership() {
    let packet = packet();
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (packet.owner_path.clone(), packet.source_blob_digest.clone()),
        ("crates/other/src/lib.rs".to_owned(), digest('e')),
    ])
    .expect("workspace tree");
    let record = packet
        .enrich_projection_record(&tree)
        .expect("enriched record");
    assert_eq!(record.projection_payload, b"fn run() {}");
    let key = ExactSelectorMerkleLookupKeyV1 {
        language_id: &record.proof.language_id,
        workspace_root_digest: &record.proof.workspace_root_digest,
        owner_path: &record.proof.owner_path,
        owner_subtree_digest: &record.proof.owner_subtree_digest,
        source_blob_digest: &record.proof.source_blob_digest,
        parser_identity_digest: &record.proof.parser_identity_digest,
        query_pack_digest: &record.proof.query_pack_digest,
        structural_selector: &record.proof.structural_selector,
        projection_mode: record.proof.projection_mode,
    };
    record.validate_warm_hit(&key).expect("validated record");
}

#[test]
fn activation_identity_digests_bind_content_not_labels() {
    let parser = derive_parser_identity_digest_v1("rs-harness", "exec-a", "registry-a");
    assert_eq!(
        parser,
        derive_parser_identity_digest_v1("rs-harness", "exec-a", "registry-a")
    );
    assert_ne!(
        parser,
        derive_parser_identity_digest_v1("rs-harness", "exec-b", "registry-a")
    );

    let query_pack = derive_query_pack_identity_digest_v1(br#"{"descriptorId":"rust-v1"}"#);
    assert_ne!(
        query_pack,
        derive_query_pack_identity_digest_v1(br#"{"descriptorId":"rust-v1","recipes":[]}"#)
    );
}
