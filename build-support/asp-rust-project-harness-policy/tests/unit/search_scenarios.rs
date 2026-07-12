use asp_rust_project_harness_policy::{
    ASP_SEARCH_SCENARIO_PACKAGE_NAME, LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID,
    SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID,
    SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID,
    SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID,
    SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID, asp_search_scenario_package,
};

#[test]
fn asp_search_scenario_package_exposes_search_performance_gates() {
    let package = asp_search_scenario_package();

    assert_eq!(package.package_name, ASP_SEARCH_SCENARIO_PACKAGE_NAME);
    assert_eq!(package.scenarios.len(), 10);

    let names = package
        .scenarios
        .iter()
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID));
    assert!(names.contains(&LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID));
    assert!(names.contains(&SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_BUSY_MISS_SCENARIO_ID
    ));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_COLD_REQUIRED_SCENARIO_ID
    ));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_READ_ONLY_CLIENT_DB_SCENARIO_ID
    ));
    assert!(names.contains(&SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID));
    assert!(names.contains(&SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID
    ));
    assert!(names.contains(&"tree-sitter-querycursor-native-hot-path"));

    let lexical = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID)
        .expect("lexical SearchFrame scenario is registered");
    assert_eq!(
        lexical.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/lexical_search_frame_graph_router_warm_path"
    );
    assert!(lexical.tags.contains(&"search-frame"));
    assert!(lexical.tags.contains(&"graph-router"));
    assert_eq!(lexical.commands.len(), 1);
    assert_eq!(lexical.commands[0].label, "warm-path-gate");
    assert_eq!(
        lexical.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "lexical_search_frame_warm_path_stays_inside_scenario_gate",
            "--",
            "--nocapture",
        ]
    );

    let chain = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID)
        .expect("source-index owner item graph chain scenario is registered");
    assert_eq!(
        chain.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_owner_item_graph_chain"
    );
    assert!(chain.tags.contains(&"source-index"));
    assert!(chain.tags.contains(&"evidence-graph"));
    assert_eq!(chain.commands[0].label, "owner-item-graph-chain");
    assert_eq!(
        chain.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_source_index_owner_item_graph_chain_is_executable",
            "--",
            "--nocapture",
        ]
    );

    let busy_miss = package
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.name
                == asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_BUSY_MISS_SCENARIO_ID
        })
        .expect("source-index busy miss scenario is registered");
    assert_eq!(
        busy_miss.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_busy_miss_overlay_skipped"
    );
    assert!(busy_miss.tags.contains(&"source-index"));
    assert!(busy_miss.tags.contains(&"busy"));
    assert!(busy_miss.tags.contains(&"overlay"));
    assert_eq!(busy_miss.commands[0].label, "busy-miss-overlay-skipped");
    assert_eq!(
        busy_miss.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_busy_source_index_miss_returns_overlay_skipped",
            "--",
            "--nocapture",
        ]
    );

    let cold_required = package
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.name
                == asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_COLD_REQUIRED_SCENARIO_ID
        })
        .expect("source-index cold-required scenario is registered");
    assert_eq!(
        cold_required.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_cold_required_overlay_skipped"
    );
    assert!(cold_required.tags.contains(&"source-index"));
    assert!(cold_required.tags.contains(&"cold-required"));
    assert!(cold_required.tags.contains(&"overlay"));
    assert_eq!(
        cold_required.commands[0].label,
        "cold-required-overlay-skipped"
    );
    assert_eq!(
        cold_required.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_cold_required_source_index_returns_overlay_skipped",
            "--",
            "--nocapture",
        ]
    );

    let read_only_client_db = package
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.name
                == asp_rust_project_harness_policy::search_scenarios::SEARCH_SOURCE_INDEX_READ_ONLY_CLIENT_DB_SCENARIO_ID
        })
        .expect("read-only source-index scenario is registered");
    assert_eq!(
        read_only_client_db.fixture_root,
        "crates/agent-semantic-client-db/tests/unit/scenarios/search_source_index_read_only_client_db"
    );
    assert!(read_only_client_db.tags.contains(&"source-index"));
    assert!(read_only_client_db.tags.contains(&"read-only"));
    assert!(read_only_client_db.tags.contains(&"turso"));
    assert_eq!(
        read_only_client_db.commands[0].label,
        "read-only-client-db-zero-write"
    );
    assert_eq!(
        read_only_client_db.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-client-db",
            "--test",
            "unit_test",
            "db_engine_source_index_lookup_succeeds_without_client_dir_write_permission",
            "--",
            "--nocapture",
        ]
    );

    let next_action = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID)
        .expect("GraphRouter next exact action scenario is registered");
    assert_eq!(
        next_action.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_graph_router_next_exact_action"
    );
    assert!(next_action.tags.contains(&"graph-router"));
    assert!(next_action.tags.contains(&"selector-ready"));
    assert_eq!(next_action.commands[0].label, "next-exact-action");
    assert_eq!(
        next_action.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_graph_router_prefers_exact_action_for_selector_ready_item",
            "--",
            "--nocapture",
        ]
    );

    let receipt = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID)
        .expect("subagent compact receipt scenario is registered");
    assert_eq!(
        receipt.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_subagent_compact_receipt"
    );
    assert!(receipt.tags.contains(&"subagent"));
    assert!(receipt.tags.contains(&"graph-route"));
    assert_eq!(receipt.commands[0].label, "compact-receipt");
    assert_eq!(
        receipt.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_subagent_receipt_is_compact_graph_route",
            "--",
            "--nocapture",
        ]
    );

    let degraded_route = package
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.name
                == asp_rust_project_harness_policy::search_scenarios::SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID
        })
        .expect("bounded degraded route scenario is registered");
    assert_eq!(
        degraded_route.fixture_root,
        "crates/agent-semantic-search/tests/unit/scenarios/search_degraded_route_bounded"
    );
    assert!(degraded_route.tags.contains(&"recovery"));
    assert!(degraded_route.tags.contains(&"bounded-fallback"));
    assert_eq!(degraded_route.commands[0].label, "bounded-degraded-route");
    assert_eq!(
        degraded_route.commands[0].argv,
        &[
            "cargo",
            "test",
            "-p",
            "agent-semantic-search",
            "--test",
            "unit_test",
            "search_flow_degraded_source_index_miss_uses_bounded_receipt_reason",
            "--",
            "--nocapture",
        ]
    );

    let tree_sitter = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == "tree-sitter-querycursor-native-hot-path")
        .expect("canonical Tree-sitter QueryCursor scenario is registered");
    assert_eq!(
        tree_sitter.fixture_root,
        "languages/rust-lang-project-harness/tests/unit/cli/query/catalog"
    );
    assert!(tree_sitter.tags.contains(&"tree-sitter"));
    assert!(tree_sitter.tags.contains(&"native-runtime"));
    assert_eq!(tree_sitter.commands.len(), 2);
    assert_eq!(tree_sitter.commands[0].label, "querycursor-packet-hot-path");
    assert_eq!(
        tree_sitter.commands[1].label,
        "querycursor-predicate-contract"
    );
}
