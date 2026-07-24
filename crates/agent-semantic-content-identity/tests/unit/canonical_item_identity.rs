use crate::canonical_item_identity::{CanonicalItemIdentityV1, CanonicalItemSelectorV1};

#[test]
fn canonical_item_identity_keeps_language_owned_scopes_generic() {
    let identity = CanonicalItemIdentityV1::new("rust", "method", "send")
        .with_scope("implementation-owner", "type", "Client")
        .with_scope("trait-owner", "trait", "Transport");
    let structural_selector = format!(
        "rust://src/client.rs#{}",
        agent_semantic_content_identity::structural_selector::encode_canonical_item_identity_path(
            &identity
        )
    );
    let selector = CanonicalItemSelectorV1::new(identity, structural_selector);

    selector.validate().expect("valid selector");
    assert_eq!(selector.scopes.len(), 2);
    assert_eq!(selector.scopes[0].relation.as_str(), "implementation-owner");
    assert_eq!(selector.scopes[1].relation.as_str(), "trait-owner");
}
