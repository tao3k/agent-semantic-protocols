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
    assert!(data_plane.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(!query_adapter.contains("ensure_runtime_generation_ready"));
    assert!(!query_adapter.contains("ensure_runtime_generation()"));
    assert!(!query_adapter.contains("RuntimeWorkspaceAdmissionCatalog::resolve_mapped"));
    assert!(!query_adapter.contains("RuntimeWorkspaceScopeResolution"));
    assert!(!data_plane.contains("connect_hook_workspace_session"));
    assert!(!data_plane.contains("connect_runtime_server_workspace_session"));
    assert!(data_plane.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(data_plane.contains("read_runtime_exact_projection"));
    assert!(!data_plane.contains("rebind_runtime_selector_overlay"));
}

#[test]
fn provider_owner_query_uses_only_the_committed_resident_runtime() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let owner_branch = daemon
        .split("RuntimeSearchServiceRequest::ProviderOwner")
        .nth(1)
        .and_then(|source| {
            source
                .split("RuntimeSearchServiceRequest::TreeSitterQuery")
                .next()
        })
        .expect("Runtime Server ProviderOwner branch");
    assert!(owner_branch.contains("ProviderRuntimeActorState::Ready"));
    assert!(
        owner_branch
            .contains("prepare_runtime_server_owner_projection_with_resident_runtime_async")
    );
    assert!(!owner_branch.contains("ProviderRuntimeProcessPeer::start"));
    assert!(!owner_branch.contains("ProviderProcessSupervisor"));
    assert!(!owner_branch.contains("prepare_runtime_server_owner_projection_with_registry_async"));
}

#[test]
fn generation_builder_uses_the_same_resident_provider_authority() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let builder = daemon
        .split("let generation_builder:")
        .nth(1)
        .and_then(|source| source.split("let owner_builder_catalog").next())
        .expect("Runtime Server generation builder");
    assert!(
        builder.contains("prepare_runtime_server_workspace_generation_with_runtime_service_async")
    );
    assert!(!builder.contains("prepare_runtime_server_workspace_generation_with_registry_async"));
    assert!(!builder.contains("ProviderProcessSupervisor"));
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
        "await_agent_facing_runtime_server_client",
        "block_on(",
        "std::thread",
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
        .find("if is_search_owner_items_query(&command_args)")
        .expect("typed pre-activation resident owner predicate");
    let owner_branch = &source[owner_route..];
    assert!(owner_branch.contains("run_search_owner_items_query_command("));
    assert!(owner_branch.contains("SearchOwnerItemsContext {"));
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

    assert!(!dispatch.contains("runtime_server_workspace_generation_client_async"));
    assert!(!dispatch.contains("WorkspaceGenerationDataPlaneClient"));
    assert!(dispatch.contains("run_runtime_provider_search_command("));
    assert!(dispatch.contains("run_search_owner_items_query_command("));
    assert!(!dispatch.contains("runtime_server_search_data_plane_async"));
    assert!(!dispatch.contains("run_asp_fast_search_command"));
    assert!(!dispatch.contains("await_agent_facing_runtime_server_client"));
    assert!(data_plane.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(data_plane.contains("read_runtime_exact_projection"));
    assert!(data_plane.contains("WorkspaceRuntimeSelectorRead"));
    assert!(!data_plane.contains("runtime_search_generation_authority"));
}

#[test]
fn search_db_facade_is_tokio_native_without_a_sync_bridge() {
    let facade = include_str!("../../../agent-semantic-client-db/src/engine/search_facade.rs");
    assert!(facade.contains("pub async fn search_source_index_documents_from_client_dir"));
    assert!(facade.contains("pub async fn search_structural_index_documents_from_client_dir"));
    for forbidden in ["block_on_db_engine", "std::thread", "thread::sleep"] {
        assert!(
            !facade.contains(forbidden),
            "search DB facade reintroduced a synchronous compatibility bridge: {forbidden}"
        );
    }
}
