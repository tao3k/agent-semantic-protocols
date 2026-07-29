use super::{owner_items_execution_route, OwnerItemsExecutionRoute};

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

#[test]
fn exact_projection_requires_one_typed_projection() {
    let source = vec!["--projection".to_owned(), "source".to_owned()];
    assert_eq!(
        crate::command::provider_direct_exact::direct_projection_kind(&source),
        Ok("source")
    );
    let skeleton = vec!["--projection=callable-skeleton".to_owned()];
    assert_eq!(
        crate::command::provider_direct_exact::direct_projection_kind(&skeleton),
        Ok("callable-skeleton")
    );
}

#[test]
fn exact_projection_rejects_legacy_flags() {
    let legacy = vec!["--code".to_owned()];
    let error = crate::command::provider_direct_exact::direct_projection_kind(&legacy)
        .expect_err("legacy exact projection must fail closed");
    assert!(error.contains("legacy --code and --names-only are unsupported"));
}

#[test]
fn typed_projection_is_not_forwarded_to_provider_cli() {
    let args = vec![
        "query".to_owned(),
        "--selector".to_owned(),
        "rust://src/lib.rs#item/function/run".to_owned(),
        "--projection".to_owned(),
        "callable-skeleton".to_owned(),
    ];
    let forwarded = crate::command::provider_direct_exact::direct_provider_process_args(&args);
    assert!(!forwarded.iter().any(|arg| arg == "--projection"));
    assert!(!forwarded.iter().any(|arg| arg == "callable-skeleton"));
    assert!(forwarded.iter().any(|arg| arg == "--selector"));
}

#[test]
fn exact_descendant_selector_uses_provider_owned_typed_route() {
    let args = vec![
        "query".to_owned(),
        "--selector".to_owned(),
        "rust://src/lib.rs#item/function/run/segment/branch/ordinal-2".to_owned(),
        "--projection".to_owned(),
        "source".to_owned(),
    ];

    assert!(
        crate::command::provider_selector::is_provider_owned_structural_selector_query(
            "rust", &args
        )
    );
    assert!(
        crate::command::provider_direct_exact::resident_canonical_item_selector(&args[2])
            .expect("valid exact descendant")
            .is_none(),
        "resident root lookup must defer exact descendants to the provider-owned typed route"
    );
}

#[test]
fn search_dispatch_cannot_materialize_provider_source_artifacts() {
    let dispatch = include_str!("../../src/command/provider_dispatch.rs");
    assert!(
        !dispatch.contains("ensure_provider_source_index_snapshot_from_activation"),
        "query-time search dispatch must not enter the provider envelope writer"
    );
    assert!(
        dispatch.contains("current_provider_source_index_snapshot_from_activation"),
        "rootDepth=0 search must capture the current provider scope in memory"
    );
}

#[test]
fn provider_query_activation_is_read_only() {
    let activation = include_str!("../../src/command/provider_activation.rs");
    assert!(
        !activation.contains("load_or_sync_activation"),
        "query commands must consume a published activation without entering synchronization"
    );
    assert!(
        activation.contains("published-language-activation-required"),
        "missing language activation must fail with a typed cold-required receipt"
    );
}
