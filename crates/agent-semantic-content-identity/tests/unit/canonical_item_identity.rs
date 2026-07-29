use crate::canonical_item_identity::{CanonicalItemIdentity, CanonicalItemSelector};

#[test]
fn canonical_item_identity_keeps_language_owned_scopes_generic() {
    let identity = CanonicalItemIdentity::new("rust", "method", "send")
        .with_scope("implementation-owner", "type", "Client")
        .with_scope("trait-owner", "trait", "Transport");
    let structural_selector = format!(
        "rust://src/client.rs#{}",
        agent_semantic_content_identity::structural_selector::encode_canonical_item_identity_path(
            &identity
        )
    );
    let selector = CanonicalItemSelector::new(identity, structural_selector);

    selector.validate().expect("valid selector");
    assert_eq!(selector.schema_id, "asp.canonical-item-selector.v1");
    assert_eq!(selector.schema_version, "1");
    assert_eq!(selector.scopes.len(), 2);
    assert_eq!(selector.scopes[0].relation.as_str(), "implementation-owner");
    assert_eq!(selector.scopes[1].relation.as_str(), "trait-owner");
}

#[test]
fn canonical_item_selector_parses_exact_descendant_root() {
    let selector = CanonicalItemSelector::parse_root_or_exact_descendant(
        "rust://src/client.rs#item/function/send/segment/branch/ordinal-2",
    )
    .expect("exact descendant selector");

    assert_eq!(selector.language_id.as_str(), "rust");
    assert_eq!(selector.kind.as_str(), "function");
    assert_eq!(selector.symbol.as_str(), "send");
    assert_eq!(
        selector.structural_selector(),
        "rust://src/client.rs#item/function/send"
    );
}

#[test]
fn canonical_item_selector_rejects_malformed_exact_descendant() {
    let error = CanonicalItemSelector::parse_root_or_exact_descendant(
        "rust://src/client.rs#item/function/send/segment/branch",
    )
    .expect_err("segment identity is required");

    assert!(error.contains("<kind>/<identity>"));
}
