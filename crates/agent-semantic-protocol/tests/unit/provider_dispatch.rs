use super::{OwnerItemsExecutionRoute, owner_items_execution_route};

#[test]
fn unsupported_native_owner_uses_registered_provider_surface() {
    assert_eq!(
        owner_items_execution_route(false, false, "org", "orgize")
            .expect("registered provider owner surface"),
        OwnerItemsExecutionRoute::RegisteredProviderSurface
    );
}

#[test]
fn declared_native_owner_uses_incremental_transport() {
    assert_eq!(
        owner_items_execution_route(true, false, "rust", "rs-harness")
            .expect("native incremental owner route"),
        OwnerItemsExecutionRoute::NativeIncremental
    );
}

#[test]
fn provider_surface_must_not_resolve_back_to_asp() {
    for native_incremental_owner in [false, true] {
        let error = owner_items_execution_route(native_incremental_owner, true, "org", "orgize")
            .expect_err("recursive provider route must fail closed");
        assert!(error.contains("reasonKind="));
        assert!(error.contains("providerId=orgize"));
    }
}
