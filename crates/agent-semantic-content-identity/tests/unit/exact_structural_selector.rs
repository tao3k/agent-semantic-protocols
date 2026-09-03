use super::{
    EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID, EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION,
    ExactCanonicalItemSelectorV1, ExactStructuralSelectorSegmentV1, ExactStructuralSelectorV1,
    ExactStructuralSelectorValidationError,
};

fn selector() -> ExactStructuralSelectorV1 {
    ExactStructuralSelectorV1 {
        schema_id: EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID.to_owned(),
        schema_version: EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION.to_owned(),
        language_id: "rust".to_owned(),
        owner_path: "crates/example/src/lib.rs".to_owned(),
        selector: "rust://crates/example/src/lib.rs#node/function/run/identity=root".to_owned(),
        generation_identity_digest: "a".repeat(64),
        parser_identity_digest: "b".repeat(64),
        query_pack_digest: "c".repeat(64),
        root_item_selector: ExactCanonicalItemSelectorV1 {
            schema_id: "asp.canonical-item-selector.v1".to_owned(),
            schema_version: "1".to_owned(),
            language_id: "rust".to_owned(),
            kind: "function".to_owned(),
            symbol: "run".to_owned(),
            scopes: Vec::new(),
            structural_selector: "rust://crates/example/src/lib.rs#item/function/run".to_owned(),
        },
        segments: Vec::new(),
    }
}

#[test]
fn validates_generation_bound_root_selector() {
    selector().validate().expect("selector should be valid");
}

#[test]
fn exact_selector_path_preserves_ordered_descendant_identity() {
    let selector = super::ExactStructuralSelectorPathV1::parse(
        "rust://crates/example/src/lib.rs#item/function/run/segment/branch/ordinal-1/segment/binding/ordinal-2",
    )
    .expect("parse exact descendant selector");

    assert_eq!(
        selector.root_selector,
        "rust://crates/example/src/lib.rs#item/function/run"
    );
    assert_eq!(selector.segments.len(), 2);
    assert_eq!(selector.segments[0].kind, "branch");
    assert_eq!(selector.segments[0].identity, "ordinal-1");
    assert_eq!(selector.segments[1].kind, "binding");
    assert_eq!(selector.segments[1].identity, "ordinal-2");
}

#[test]
fn rejects_line_based_segment_identity() {
    let mut value = selector();
    value.segments.push(ExactStructuralSelectorSegmentV1 {
        relation: "contains".to_owned(),
        kind: "arm".to_owned(),
        identity: "line:42".to_owned(),
        label: None,
    });
    assert_eq!(
        value.validate(),
        Err(ExactStructuralSelectorValidationError::InvalidSegmentIdentity { index: 0 })
    );
}
