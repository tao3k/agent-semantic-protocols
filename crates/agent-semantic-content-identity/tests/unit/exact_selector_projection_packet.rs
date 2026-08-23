use super::build_exact_selector_projection_packet_v1;
use crate::exact_selector_merkle::{ExactProjectionModeV1, canonical_content_digest};

#[test]
fn builder_binds_source_parser_facts_and_projection_bytes() {
    use crate::canonical_item_identity::{CanonicalItemIdentity, CanonicalItemSelector};

    let parser_digest = canonical_content_digest(b"parser", &[b"rs-harness"]);
    let query_pack_digest = canonical_content_digest(b"query-pack", &[b"rust"]);
    let structural_selector = "rust://src/lib.rs#item/function/example";
    let language_id =
        crate::exact_selector_projection_packet::ProjectionPacketLanguageIdV1::from("rust");
    let provider_id =
        crate::exact_selector_projection_packet::ProjectionPacketProviderIdV1::from("rs-harness");
    let owner_path =
        crate::exact_selector_projection_packet::ProjectionPacketOwnerPathV1::from("src/lib.rs");
    let typed_structural_selector =
        crate::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1::from(
            structural_selector,
        );
    let canonical_item_selector = CanonicalItemSelector::new(
        CanonicalItemIdentity::new("rust", "function", "example"),
        structural_selector,
    );
    let packet = build_exact_selector_projection_packet_v1(
        crate::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
            source_byte_start: 0,
            source_byte_end: 16,
            language_id: &language_id,
            provider_id: &provider_id,
            canonical_item_selector: canonical_item_selector.clone(),
            parser_identity_digest: &parser_digest,
            query_pack_digest: &query_pack_digest,
            owner_path: &owner_path,
            structural_selector: &typed_structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            source: b"fn example() {}\n",
            normalized_parser_facts: br#"{"kind":"fn","name":"example"}"#,
            projection: b"fn example() {}\n",
        },
    );
    assert_eq!(packet.schema_version, "1");
    assert_eq!(
        packet.projection_payload_base64.as_str(),
        "Zm4gZXhhbXBsZSgpIHt9Cg=="
    );

    let changed = build_exact_selector_projection_packet_v1(
        crate::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
            source_byte_start: 0,
            source_byte_end: 16,
            language_id: &language_id,
            provider_id: &provider_id,
            canonical_item_selector,
            parser_identity_digest: &parser_digest,
            query_pack_digest: &query_pack_digest,
            owner_path: &owner_path,
            structural_selector: &typed_structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            source: b"fn example() { todo!() }\n",
            normalized_parser_facts: br#"{"kind":"fn","name":"example"}"#,
            projection: b"fn example() { todo!() }\n",
        },
    );
    assert_ne!(packet.source_blob_digest, changed.source_blob_digest);
    assert_ne!(packet.parser_fact_digest, changed.parser_fact_digest);
}
