use super::build_exact_selector_projection_packet_v1;
use crate::exact_selector_merkle::{ExactProjectionModeV1, canonical_content_digest_v1};

#[test]
fn builder_binds_source_parser_facts_and_projection_bytes() {
    use crate::canonical_item_identity::{CanonicalItemIdentityV1, CanonicalItemSelectorV1};

    let parser_digest = canonical_content_digest_v1(b"parser", &[b"rs-harness"]);
    let query_pack_digest = canonical_content_digest_v1(b"query-pack", &[b"rust"]);
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
    let canonical_item_selector = CanonicalItemSelectorV1::new(
        CanonicalItemIdentityV1::new("rust", "function", "example"),
        structural_selector,
    );
    let packet = build_exact_selector_projection_packet_v1(
        &language_id,
        &provider_id,
        canonical_item_selector.clone(),
        &parser_digest,
        &query_pack_digest,
        &owner_path,
        &typed_structural_selector,
        ExactProjectionModeV1::Code,
        b"fn example() {}\n",
        br#"{"kind":"fn","name":"example"}"#,
        b"fn example() {}\n",
    );
    assert_eq!(packet.schema_version, "1");
    assert_eq!(
        packet.projection_payload_base64.as_str(),
        "Zm4gZXhhbXBsZSgpIHt9Cg=="
    );

    let changed = build_exact_selector_projection_packet_v1(
        &language_id,
        &provider_id,
        canonical_item_selector,
        &parser_digest,
        &query_pack_digest,
        &owner_path,
        &typed_structural_selector,
        ExactProjectionModeV1::Code,
        b"fn example() { todo!() }\n",
        br#"{"kind":"fn","name":"example"}"#,
        b"fn example() { todo!() }\n",
    );
    assert_ne!(packet.source_blob_digest, changed.source_blob_digest);
    assert_ne!(packet.parser_fact_digest, changed.parser_fact_digest);
}
