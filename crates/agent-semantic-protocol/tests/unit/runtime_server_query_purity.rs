#[test]
fn query_data_plane_never_invokes_generation_reconciliation() {
    let query_adapter = include_str!("../../src/server/runtime_server.rs");
    let data_plane = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    let readiness = include_str!("../../src/server/runtime_server_generation.rs");
    assert!(
        !query_adapter.contains("repair_runtime_generation_locator"),
        "query adapter reintroduced supervisor-owned generation reconciliation"
    );
    assert!(
        !readiness.contains("ensure_runtime_generation_admitted_for_projection_async"),
        "generation owner reintroduced the non-terminal admission bridge"
    );
    assert!(!data_plane.contains("ensure_runtime_generation_ready"));
    assert!(data_plane.contains("runtime_server_workspace_session_async"));
    assert!(!data_plane.contains("connect_hook_workspace_session"));
    assert!(!data_plane.contains("runtime_generation_pointer_path"));
    assert!(!data_plane.contains("connect_runtime_server_workspace_session"));
}

#[test]
fn graph_turbo_ranking_is_a_read_only_generation_consumer() {
    let graph = include_str!("../../src/command/graph.rs");
    assert!(graph.contains("runtime_server_workspace_session_async"));
    assert!(!graph.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(!graph.contains("ensure_runtime_generation_ready"));
}

#[test]
fn exact_projection_is_a_read_only_generation_consumer() {
    let source = include_str!("../../src/command/provider_resident_exact.rs");
    for forbidden in [
        "ensure_runtime_generation_owner_ready",
        "owner-freshness-before",
        "owner-freshness-ready",
        "publish_owner_overlay",
        "tombstone_owner_overlay",
        "ensure_runtime_generation_ready",
    ] {
        assert!(
            !source.contains(forbidden),
            "exact projection reintroduced query-time generation mutation: {forbidden}"
        );
    }
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

#[test]
fn search_adapter_never_decodes_the_complete_resident_generation() {
    let dispatch = include_str!("../../src/command/provider_dispatch.rs");
    let data_plane = include_str!("../../src/server/runtime_server_generation_data_plane.rs");
    let source = include_str!("../../src/command/search_pipe_source.rs");
    let facts = include_str!("../../src/command/search_pipe_provider_facts.rs");

    assert!(!dispatch.contains("runtime_server_workspace_generation_client_async"));
    assert!(!dispatch.contains("WorkspaceGenerationDataPlaneClient"));
    assert!(dispatch.contains("runtime_server_search_data_plane_async"));
    assert!(data_plane.contains("runtime_search_generation_authority"));
    assert!(source.contains("read_source_index"));
    let projection = include_str!(
        "../../../agent-semantic-client-db/src/runtime_server_workspace/search_index_projection.rs"
    );
    assert!(projection.contains("read_graph_facts"));
    assert!(
        !source.contains(".lease().read_source_index"),
        "short-lived search reintroduced a complete process-local generation lease"
    );
    assert!(
        !facts.contains(".relations_from("),
        "short-lived search reintroduced process-local relation graph decoding"
    );
}
