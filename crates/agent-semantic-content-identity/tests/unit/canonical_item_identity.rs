use crate::canonical_item_identity::{CanonicalItemIdentityV1, CanonicalItemSelectorV1};

#[test]
fn canonical_item_identity_keeps_language_owned_scopes_generic() {
    let identity = CanonicalItemIdentityV1::new("rust", "method", "send")
        .with_scope("implementation-owner", "type", "Client")
        .with_scope("trait-owner", "trait", "Transport");
    let selector = CanonicalItemSelectorV1::new(
        identity,
        "rust://src/client.rs#item/method/<Client as Transport>::send",
    );

    selector.validate().expect("valid selector");
    assert_eq!(selector.scopes.len(), 2);
    assert_eq!(selector.scopes[0].relation, "implementation-owner");
    assert_eq!(selector.scopes[1].relation, "trait-owner");
}
