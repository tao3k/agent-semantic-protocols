#[test]
fn language_facade_uses_one_persistent_http_session_and_catalog_typed_params() {
    let dispatch = include_str!("../../src/command/provider_dispatch.rs");

    assert!(dispatch.contains("RuntimeHttpClient::new("));
    assert!(dispatch.contains("client.open_session().await?"));
    assert!(!dispatch.contains("RuntimeProviderRouteIntent"));
    assert!(dispatch.contains("session.request_route("));
    assert!(dispatch.contains("session.shutdown().await"));

    for forbidden in [
        "runtime_server_ensure_workspace_async",
        "runtime_server_workspace_session_async",
        "provider_resident_exact",
        "run_search_owner_items_query_command",
        "provider_operation(",
        "run_asp_fast_search_command",
        "await_agent_facing_runtime_server_client",
        "block_on(",
        "std::thread",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "language facade reintroduced a legacy or query-time lifecycle path: {forbidden}"
        );
    }
}

#[test]
fn generation_builds_bind_the_current_installed_capability_catalog() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");

    assert!(daemon.contains("load_runtime_provider_artifacts(&state_home).await?"));
    assert!(!daemon.contains("catalog advanced after daemon admission"));
}

#[test]
fn cli_adapter_emits_route_semantics_instead_of_forwarding_argv() {
    let dispatch = include_str!("../../src/command/provider_dispatch.rs");

    assert!(dispatch.contains("runtime_search_intent(&provider_args)?"));
    assert!(dispatch.contains("runtime_query_intent(&exact_provider_args)?"));
    assert!(dispatch.contains("runtime_owner_intent(&owner_args)?"));
    assert!(dispatch.contains("\"selector\": selector"));
    assert!(dispatch.contains("\"ownerPath\": owner_path"));
    assert!(dispatch.contains("\"query\": queries.join(\" \")"));
    assert!(!dispatch.contains("serde_json::json!({\"argv\""));
}

#[test]
fn runtime_dispatch_reads_only_the_published_immutable_generation() {
    let dispatcher =
        include_str!("../../../agent-semantic-runtime-server/src/runtime_asp_client.rs");
    let generation = include_str!("../../../agent-semantic-runtime-server/src/query_generation.rs");

    assert!(dispatcher.contains(".get(request.workspace_identity.as_str())"));
    assert!(dispatcher.contains(".read_source_index("));
    assert!(dispatcher.contains(".read_runtime_owner("));
    assert!(dispatcher.contains(".read_runtime_selector("));
    assert!(generation.contains("RuntimeQueryGenerationState"));
    assert!(generation.contains("Failed(Arc<str>)"));
    assert!(generation.contains("RuntimeResidentReadClient::open("));

    for forbidden in [
        "provider_operation(",
        "ensure_workspace",
        "build_generation",
        "await_ready",
        "thread::sleep",
        "tokio::time::sleep",
    ] {
        assert!(
            !dispatcher.contains(forbidden),
            "Ready query dispatch reintroduced lifecycle work: {forbidden}"
        );
    }
}

#[test]
fn old_cli_exact_and_generation_data_planes_are_not_declared() {
    let command_modules = include_str!("../../src/command/mod.rs");
    let crate_modules = include_str!("../../src/lib.rs");
    let server_modules = include_str!("../../src/server/mod.rs");

    for forbidden in [
        "provider_exact_args",
        "provider_resident_exact",
        "search_owner_items",
    ] {
        assert!(!command_modules.contains(forbidden));
    }
    for forbidden in [
        "exact_projection_diagnostic",
        "exact_projection_trace",
        "resident_exact_projection",
    ] {
        assert!(!crate_modules.contains(forbidden));
    }
    assert!(!server_modules.contains("runtime_server_generation_data_plane"));
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

#[test]
fn query_generation_lane_uses_explicit_daemon_shutdown() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");

    assert!(daemon.contains("let mut generation_shutdown = client_http_shutdown.subscribe();"));
    assert!(daemon.contains("changed = generation_shutdown.changed()"));
    assert!(daemon.contains("let _ = client_http_shutdown.send(true);"));
    assert!(daemon.contains("let _ = generation_task.join().await;"));

    let shutdown = daemon
        .find("let _ = client_http_shutdown.send(true);")
        .expect("explicit daemon shutdown publication");
    let join = daemon
        .find("let _ = generation_task.join().await;")
        .expect("generation lane join");
    assert!(
        shutdown < join,
        "generation shutdown must linearize before join"
    );
}

#[test]
fn runtime_owner_identity_is_digest_addressed_not_a_mutable_entrypoint() {
    let adapter = include_str!("../../src/server/runtime_server_wire_adapter.rs");

    assert!(adapter.contains("program: runtime_artifact"));
    assert!(adapter.contains("expected_executable: runtime_artifact.clone()"));
    assert!(adapter.contains("Ok(resolved)"));
    assert!(!adapter.contains("Ok(stable_entry)"));
}

#[test]
fn runtime_readiness_has_no_wall_clock_timeout_policy() {
    let lifecycle = include_str!("../../src/server/runtime_server.rs");

    assert!(!lifecycle.contains("STARTUP_DEADLINE"));
    assert!(!lifecycle.contains("runtime-server-readiness-deadline-exceeded"));
    assert!(!lifecycle.contains("tokio::time::timeout(\n        STARTUP_DEADLINE"));
}

#[test]
fn identity_handoff_admits_the_new_active_artifact_not_the_retiring_invoker() {
    let adapter = include_str!("../../src/server/runtime_server_wire_adapter.rs");
    let handoff = include_str!("../../src/server/runtime_server_identity_handoff.rs");

    assert!(handoff.contains("reconcile_healthy_runtime_server_after_identity_handoff"));
    assert!(adapter.contains("ensure_runtime_server_after_identity_handoff"));
    assert!(adapter.contains("supervisor_request_for_active_artifact"));
    assert!(adapter.contains("&runtime_artifact,\n        receipt,"));
    assert!(adapter.contains("&current_exe,\n        &receipt,"));
}

#[test]
fn binary_install_only_migrates_state_home_agent_authority() {
    let install = include_str!("../../src/command/install_provider_binary.rs");

    assert!(install.contains("synchronize_embedded_agent_state_config"));
    assert!(!install.contains("synchronize_embedded_agent_config("));
    assert!(install.contains("agentConfigCoupling=binary-content"));
}
