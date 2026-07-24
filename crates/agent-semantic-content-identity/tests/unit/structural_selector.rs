use agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1;
use agent_semantic_content_identity::structural_selector::{
    decode_canonical_item_identity_path, encode_canonical_item_identity_path,
};

#[test]
fn canonical_item_identity_path_round_trips_ordered_cfg_scopes() {
    let identity = CanonicalItemIdentityV1::new("rust", "function", "run_search_view").with_scope(
        "conditional-compilation",
        "cfg",
        "not(feature = \"semantic-search-json\")",
    );

    let encoded = encode_canonical_item_identity_path(&identity);
    let language_id = crate::structural_selector::StructuralSelectorLanguageId::from("rust");
    let encoded_identity_path =
        crate::structural_selector::CanonicalItemIdentityPath::from(encoded.as_str());
    assert_eq!(
        encoded,
        "item/function/run_search_view/scope/conditional-compilation/cfg/not%28feature%20%3D%20%22semantic-search-json%22%29"
    );
    assert_eq!(
        decode_canonical_item_identity_path(&language_id, &encoded_identity_path).expect("decode"),
        identity
    );
}

#[test]
fn canonical_item_identity_path_rejects_lowercase_percent_escapes() {
    let language_id = crate::structural_selector::StructuralSelectorLanguageId::from("rust");
    let identity_path = crate::structural_selector::CanonicalItemIdentityPath::from(
        "item/function/run_search_view/scope/conditional-compilation/cfg/feature%3djson",
    );
    let error = decode_canonical_item_identity_path(
        &language_id,
        &identity_path,
    )
    .expect_err("lowercase percent escape must fail");

    assert!(error.to_string().contains("uppercase hex"));
}
