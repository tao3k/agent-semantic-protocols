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
    assert_eq!(package.scenarios.len(), 6);

    let names = package
        .scenarios
        .iter()
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID));
    assert!(names.contains(&LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID));
    assert!(names.contains(&SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID));
    assert!(names.contains(&SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID));
    assert!(names.contains(&SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID
    ));

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
}
