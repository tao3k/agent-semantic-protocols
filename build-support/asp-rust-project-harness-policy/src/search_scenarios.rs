//! Search-owned ASP Rust harness scenarios.

use crate::AspRustProjectHarnessScenarioPackage;

/// Package name for ASP search scenario gates.
pub const ASP_SEARCH_SCENARIO_PACKAGE_NAME: &str = "agent-semantic-search";

/// Package-local coverage monitor scenario for search surfaces.
pub const SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID: &str =
    "search-package-linear-performance-monitoring";

/// Warm lexical `SearchFrame` and `GraphRouter` performance scenario.
pub const LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID: &str =
    "lexical-search-frame-graph-router-warm-path";

/// Source-index evidence chain from owner evidence to graph nodes.
pub const SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID: &str =
    "search-source-index-owner-item-graph-chain";

/// GraphRouter next-action policy for selector-ready evidence.
pub const SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID: &str =
    "search-graph-router-next-exact-action";

/// Compact graph-route receipt contract for ASP search subagents.
pub const SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID: &str = "search-subagent-compact-receipt";

/// Bounded route contract for source-index and owner-item miss recovery.
pub const SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID: &str = "search-degraded-route-bounded";

/// Builds the ASP-owned search scenario package consumed by Rust harness policy.
#[must_use]
pub fn asp_search_scenario_package() -> AspRustProjectHarnessScenarioPackage {
    crate::asp_rust_project_harness_scenario_package!(
        package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        scenarios: [
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Search package surfaces stay covered by package-local benchmark metadata.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_package_linear_performance_monitoring",
                tags: ["search", "performance", "package-monitoring"],
                commands: [
                    {
                        label: "surface-coverage",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-search",
                            "search_package_linear_performance_monitoring_covers_all_unit_surfaces",
                        ]
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Lexical SearchFrame warm evidence routes through GraphRouter without provider or finder startup.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/lexical_search_frame_graph_router_warm_path",
                tags: ["search", "performance", "search-frame", "graph-router"],
                commands: [
                    {
                        label: "warm-path-gate",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Source-index evidence projects to executable owner, item, and hot graph nodes.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_owner_item_graph_chain",
                tags: ["search", "source-index", "evidence-graph", "owner-item"],
                commands: [
                    {
                        label: "owner-item-graph-chain",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "GraphRouter chooses exact selector actions and rejects seed escape after selector-ready evidence.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_graph_router_next_exact_action",
                tags: ["search", "graph-router", "next-action", "selector-ready"],
                commands: [
                    {
                        label: "next-exact-action",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "ASP search subagents return compact graph-route receipts without source bodies, confidence fields, or line-range selectors.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_subagent_compact_receipt",
                tags: ["search", "subagent", "receipt", "graph-route"],
                commands: [
                    {
                        label: "compact-receipt",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Source-index and owner-item misses return an explicit bounded GraphRoute receipt instead of silent broad finder fallback.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_degraded_route_bounded",
                tags: ["search", "graph-router", "recovery", "bounded-fallback"],
                commands: [
                    {
                        label: "bounded-degraded-route",
                        argv: [
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
                    },
                ],
            ),
        ],
    )
}
