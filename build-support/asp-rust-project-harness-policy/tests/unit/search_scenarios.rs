// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use asp_rust_project_harness_policy::ASP_SEARCH_SCENARIO_PACKAGE_NAME;
use asp_rust_project_harness_policy::LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID;
use asp_rust_project_harness_policy::SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID;
use asp_rust_project_harness_policy::SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID;
use asp_rust_project_harness_policy::SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID;
use asp_rust_project_harness_policy::SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID;
use asp_rust_project_harness_policy::asp_search_scenario_package;

#[test]
fn asp_search_scenario_package_exposes_search_performance_gates() {
    let package = asp_search_scenario_package();

    assert_eq!(package.package_name, ASP_SEARCH_SCENARIO_PACKAGE_NAME);

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
    assert!(names.contains(&"topology-index-v1"));
    assert!(names.contains(&SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID));
    assert!(names.contains(&SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID
    ));
    assert!(names.contains(
        &asp_rust_project_harness_policy::search_scenarios::RUNTIME_SEARCH_TOKIO_RESOURCE_LIFECYCLE_SCENARIO_ID
    ));
    assert!(names.contains(&"tree-sitter-querycursor-native-hot-path"));

    let runtime_resources = package
        .scenarios
        .iter()
        .find(|scenario| {
            scenario.name
                == asp_rust_project_harness_policy::search_scenarios::RUNTIME_SEARCH_TOKIO_RESOURCE_LIFECYCLE_SCENARIO_ID
        })
        .expect("Runtime Search resource lifecycle Scenario is registered");
    let benchmark = runtime_resources
        .benchmark
        .as_ref()
        .expect("Runtime Search Scenario owns executable benchmark metadata");
    assert_eq!(benchmark.measure_iterations, 128);
    assert!(
        benchmark
            .metrics
            .iter()
            .any(|metric| metric.name == "queue_wait_micros")
    );

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

    let next_action = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID)
        .expect("GraphRouter next exact action scenario is registered");
    assert!(next_action.tags.contains(&"graph-router"));
    assert!(next_action.tags.contains(&"selector-ready"));

    let receipt = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID)
        .expect("subagent compact receipt scenario is registered");
    assert!(receipt.tags.contains(&"subagent"));
    assert!(receipt.tags.contains(&"graph-route"));

    let tree_sitter = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == "tree-sitter-querycursor-native-hot-path")
        .expect("canonical Tree-sitter QueryCursor scenario is registered");
    assert_eq!(
        tree_sitter.fixture_root,
        "languages/asp-rust/tests/unit/cli/query/catalog"
    );
    assert!(tree_sitter.tags.contains(&"tree-sitter"));
    assert!(tree_sitter.tags.contains(&"native-runtime"));
    assert_eq!(tree_sitter.commands.len(), 2);
}
