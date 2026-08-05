#[test]
fn query_data_plane_never_invokes_generation_reconciliation() {
    let query_adapter = include_str!("../../src/server/runtime_server.rs");
    assert!(
        !query_adapter.contains("repair_runtime_generation_locator"),
        "query adapter reintroduced supervisor-owned generation reconciliation"
    );
    assert!(
        !query_adapter.contains("ensure_runtime_generation_admitted_for_projection_async"),
        "query adapter reintroduced the non-terminal generation admission bridge"
    );
    assert_eq!(
        query_adapter
            .matches("ensure_runtime_generation_ready_for_projection_async")
            .count(),
        2,
        "source and exact projection misses must both await the typed Ready gate"
    );
}

#[test]
fn owner_items_is_a_pre_activation_resident_read() {
    let source = include_str!("../../src/command/provider_dispatch.rs");
    let owner_route = source
        .find("owner-resident-read-admitted")
        .expect("pre-activation resident owner route");
    let activation_load = source
        .find("load_activation_for_language(")
        .expect("activation path for provider-backed commands");
    assert!(
        owner_route < activation_load,
        "owner-items must route before activation construction"
    );
    assert!(source[owner_route..activation_load].contains("provider_context: None"));
    for legacy in [
        "OwnerItemsExecutionRoute",
        "owner-provider-surface-admitted",
        "owner-native-incremental-admitted",
        "provider_invokes_asp_facade",
    ] {
        assert!(
            !source.contains(legacy),
            "legacy owner dispatch surface reintroduced: {legacy}"
        );
    }
}
