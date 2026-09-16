// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

/// Busy source-index miss route must not fall through to search overlay.
pub const SEARCH_SOURCE_INDEX_BUSY_MISS_SCENARIO_ID: &str =
    "search-source-index-busy-miss-overlay-skipped";

/// Cold source-index schemas require an explicit rebuild and must not fall through to overlay.
pub const SEARCH_SOURCE_INDEX_COLD_REQUIRED_SCENARIO_ID: &str =
    "search-source-index-cold-required-overlay-skipped";

/// Read-only source-index lookup must work while the client directory rejects writes.
pub const SEARCH_SOURCE_INDEX_READ_ONLY_CLIENT_DB_SCENARIO_ID: &str =
    "search-source-index-read-only-client-db-zero-write";

/// Content-addressed parser products survive generation rebinding while
/// retaining owner-local invalidation.
pub const PARSER_ARTIFACT_CONTENT_REUSE_SCENARIO_ID: &str = "parser-artifact-content-reuse";

/// The first caller and every identical concurrent caller await one retained
/// generation-owned terminal without a caller retry.
pub const FIRST_CALL_SINGLE_FLIGHT_TERMINAL_SCENARIO_ID: &str = "first-call-single-flight-terminal";

/// Merkle-qualified live-memory code search must never open Turso on the warm path.
pub const CODE_SEARCH_MERKLE_MEMORY_WARM_PATH_SCENARIO_ID: &str =
    "code-search-merkle-memory-warm-path";

/// Merkle-qualified code search over one resident Turso 0.7 read session.
pub const CODE_SEARCH_TURSO_RESIDENT_SESSION_WARM_PATH_SCENARIO_ID: &str =
    "code-search-turso-resident-session-warm-path";

/// GraphRouter next-action policy for selector-ready evidence.
pub const SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID: &str =
    "search-graph-router-next-exact-action";

/// Compact graph-route receipt contract for ASP search subagents.
pub const SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID: &str = "search-subagent-compact-receipt";

/// Bounded route contract for source-index and owner-item miss recovery.
pub const SEARCH_DEGRADED_ROUTE_BOUNDED_SCENARIO_ID: &str = "search-degraded-route-bounded";

/// Runtime Search stages share daemon CPU and memory admission.
pub const RUNTIME_SEARCH_TOKIO_RESOURCE_LIFECYCLE_SCENARIO_ID: &str =
    "runtime-search-tokio-resource-lifecycle";

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
                name: SEARCH_SOURCE_INDEX_BUSY_MISS_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Busy source-index misses return immediately and skip search overlay fallback.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_busy_miss_overlay_skipped",
                tags: ["search", "source-index", "performance", "busy", "overlay"],
                commands: [
                    {
                        label: "busy-miss-overlay-skipped",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_SOURCE_INDEX_COLD_REQUIRED_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Cold source-index schemas require rebuild and skip search overlay fallback.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_source_index_cold_required_overlay_skipped",
                tags: ["search", "source-index", "performance", "cold-required", "overlay"],
                commands: [
                    {
                        label: "cold-required-overlay-skipped",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_SOURCE_INDEX_READ_ONLY_CLIENT_DB_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Source-index lookup stays bounded and produces no client-directory writes.",
                fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/search_source_index_read_only_client_db",
                tags: ["search", "source-index", "performance", "read-only", "turso"],
                commands: [
                    {
                        label: "read-only-client-db-zero-write",
                        argv: [
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
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: PARSER_ARTIFACT_CONTENT_REUSE_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Parser artifacts bind owner content and applicable auxiliary cuts independently from generation proof.",
                fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/parser_artifact_content_reuse",
                tags: ["search", "parser-artifact", "content-addressed", "incremental", "performance"],
                commands: [
                    {
                        label: "content-reuse-work-metrics",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client-db",
                            "--lib",
                            "server_source_index::projection::tests::parser_artifact_content_reuse_is_scenario_measured",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "provider-free-generation-rebind",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client-db",
                            "--lib",
                            "server_source_index::async_rebuild::tests::unchanged_owner_reuses_parser_artifact_without_provider_runtime",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
                benchmark: {
                    harness: "libtest",
                    test: "parser_artifact_content_reuse_is_scenario_measured",
                    snapshot: "parser_artifact_content_reuse_v1",
                    target_total: "50us",
                    max_total: "5ms",
                    regression_budget: "50us",
                    memory_budget_bytes: 65_536,
                    target_rationale: "Two auxiliary owners are hashed once, then owner-local cuts are derived without provider startup or generation-wide invalidation.",
                    warmup_iterations: 16,
                    measure_iterations: 128,
                    metrics: [
                        { name: "auxiliary_owner_hash_count", unit: "owners", kind: Exact, target: 2 },
                        { name: "affected_owner_count", unit: "owners", kind: Exact, target: 1 },
                        { name: "unrelated_owner_invalidation_count", unit: "owners", kind: Exact, target: 0 },
                        { name: "provider_process_count", unit: "processes", kind: Exact, target: 0 }
                    ]
                }
            ),
            crate::asp_rust_project_harness_scenario!(
                name: FIRST_CALL_SINGLE_FLIGHT_TERMINAL_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "The initial Search/Query caller and concurrent joiners receive one retained generation-owned terminal without retrying.",
                fixture_root: "crates/agent-semantic-runtime-server/tests/unit/scenarios/first_call_single_flight_terminal",
                tags: ["search", "query", "runtime", "single-flight", "first-result"],
                commands: [
                    {
                        label: "first-call-terminal",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-runtime-server",
                            "query_generation::tests::first_call_single_flight_terminal_is_scenario_measured",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
                benchmark: {
                    harness: "libtest",
                    test: "first_call_single_flight_terminal_is_scenario_measured",
                    snapshot: "first_call_single_flight_terminal_v1",
                    target_total: "250us",
                    max_total: "10ms",
                    regression_budget: "250us",
                    memory_budget_bytes: 1_048_576,
                    target_rationale: "Identical callers perform one atomic claim and event fan-out; timing is diagnostic while work counters are authoritative.",
                    warmup_iterations: 16,
                    measure_iterations: 128,
                    metrics: [
                        { name: "search_computation_claim_count", unit: "claims", kind: Exact, target: 1 },
                        { name: "query_computation_claim_count", unit: "claims", kind: Exact, target: 1 },
                        { name: "terminal_waiter_count", unit: "waiters", kind: Exact, target: 64 },
                        { name: "generation_owned_task_count", unit: "tasks", kind: Exact, target: 2 },
                        { name: "caller_retry_count", unit: "retries", kind: Exact, target: 0 },
                        { name: "public_building_terminal_count", unit: "terminals", kind: Exact, target: 0 }
                    ]
                }
            ),
            crate::asp_rust_project_harness_scenario!(
                name: CODE_SEARCH_MERKLE_MEMORY_WARM_PATH_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Merkle-qualified live-memory code search stays below one millisecond p95 without opening Turso or starting providers.",
                fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/code_search_merkle_memory_warm_path",
                tags: ["search", "code-search", "performance", "merkle", "memory", "turso"],
                commands: [
                    {
                        label: "merkle-memory-warm-path-gate",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client-db",
                            "--test",
                            "performance_test",
                            "code_search_merkle_memory_warm_path_is_a_strong_gate",
                            "--",
                            "--nocapture",
                        ]
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: CODE_SEARCH_TURSO_RESIDENT_SESSION_WARM_PATH_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Merkle-qualified code search reuses one resident Turso 0.7 read session without reconnecting or starting providers.",
                fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/code_search_turso_resident_session_warm_path",
                tags: ["search", "code-search", "performance", "merkle", "turso", "resident-session"],
                commands: [
                    {
                        label: "turso-resident-session-warm-path-gate",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client-db",
                            "--test",
                            "performance_test",
                            "code_search_turso_resident_session_warm_path_is_a_strong_gate",
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
            crate::asp_rust_project_harness_scenario!(
                name: RUNTIME_SEARCH_TOKIO_RESOURCE_LIFECYCLE_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Resident retrieval and parser grounding share the Runtime daemon resource supervisor while retaining intermediate-result memory reservations.",
                fixture_root: "crates/agent-semantic-workspace-scheduler/tests/unit/scenarios/runtime_search_tokio_resource_lifecycle",
                tags: ["search", "runtime", "tokio", "performance", "resource-budget"],
                commands: [
                    {
                        label: "unified-resource-lifecycle",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-workspace-scheduler",
                            "runtime_search_resource_lifecycle_is_scenario_measured",
                            "--",
                            "--nocapture",
                        ]
                    },
                ],
                benchmark: {
                    harness: "libtest",
                    test: "runtime_search_resource_lifecycle_is_scenario_measured",
                    snapshot: "runtime_search_tokio_resource_lifecycle_v1",
                    target_total: "250us",
                    max_total: "5ms",
                    regression_budget: "250us",
                    memory_budget_bytes: 2_097_152,
                    target_rationale: "Two resident CPU stages require O(1) permit operations; retained intermediate memory is charged until Search projection completes.",
                    warmup_iterations: 16,
                    measure_iterations: 128,
                    metrics: [
                        { name: "queue_wait_micros", unit: "microseconds", kind: Maximum, target: 5000 },
                        { name: "admitted_cpu", unit: "lanes", kind: Exact, target: 1 },
                        { name: "peak_admitted_memory_bytes", unit: "bytes", kind: Exact, target: 2_097_152 },
                        { name: "completed_stage_count", unit: "count", kind: Exact, target: 2 }
                    ]
                }
            ),
            crate::asp_rust_project_harness_scenario!(
                name: "tree-sitter-querycursor-native-hot-path",
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Canonical Tree-sitter QueryCursor execution keeps predicate semantics and bounded native hot-path metrics visible.",
                fixture_root: "languages/asp-rust/tests/unit/cli/query/catalog",
                tags: ["search", "query", "tree-sitter", "performance", "native-runtime"],
                commands: [
                    {
                        label: "querycursor-packet-hot-path",
                        argv: [
                            "cargo",
                            "test",
                            "--manifest-path",
                            "languages/asp-rust/Cargo.toml",
                            "--features",
                            "cli",
                            "--test",
                            "unit_test",
                            "tree_sitter_query_json_projects_matches_and_native_enrichment",
                            "--",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "querycursor-predicate-contract",
                        argv: [
                            "cargo",
                            "test",
                            "--manifest-path",
                            "languages/asp-rust/Cargo.toml",
                            "--features",
                            "cli",
                            "--test",
                            "unit_test",
                            "cli::query::catalog::stdout::predicates",
                            "--",
                            "--nocapture",
                        ]
                    },
                ],
            ),
        ],
    )
}
