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

/// Detailed topology materialization reads only the bounded candidate owner
/// frontier and never projects the complete workspace on a request path.
pub const CANDIDATE_TOPOLOGY_OWNER_SCOPE_SCENARIO_ID: &str = "candidate-topology-owner-scope";

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

/// Resident GREP semantic matrix and explicit external-rg qualification lane.
pub const RUNTIME_RESIDENT_GREP_SEMANTICS_SCENARIO_ID: &str = "runtime-resident-grep-semantics";

/// Human Query must render callable skeletons without copying the complete wire envelope.
pub const QUERY_CALLABLE_SKELETON_COMPACT_PRESENTATION_SCENARIO_ID: &str =
    "query-callable-skeleton-compact-presentation";

/// Search fan-in uses one owner-support index and rejects graph-only owner promotion.
pub const SEARCH_RESULT_OWNER_SUPPORT_INDEX_SCENARIO_ID: &str = "search-result-owner-support-index";

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
                name: CANDIDATE_TOPOLOGY_OWNER_SCOPE_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Detailed topology materialization projects only the exact candidate owner scope from a large resident generation.",
                fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/candidate_topology_owner_scope",
                tags: ["search", "topology", "candidate-scope", "performance"],
                commands: [
                    {
                        label: "candidate-scope-work-metrics",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client-db",
                            "--lib",
                            "runtime_server_workspace::registry::canonical_publication::tests::candidate_topology_owner_scope_is_scenario_measured",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
                benchmark: {
                    harness: "libtest",
                    test: "candidate_topology_owner_scope_is_scenario_measured",
                    snapshot: "candidate_topology_owner_scope_v1",
                    target_total: "250us",
                    max_total: "10ms",
                    regression_budget: "250us",
                    memory_budget_bytes: 262_144,
                    target_rationale: "A request over two owners performs two indexed owner reads regardless of total workspace owner cardinality; timing is diagnostic and exact work counters are authoritative.",
                    warmup_iterations: 16,
                    measure_iterations: 128,
                    metrics: [
                        { name: "workspace_owner_count", unit: "owners", kind: Exact, target: 4096 },
                        { name: "requested_owner_count", unit: "owners", kind: Exact, target: 2 },
                        { name: "returned_owner_count", unit: "owners", kind: Exact, target: 2 },
                        { name: "unrequested_owner_projection_count", unit: "owners", kind: Exact, target: 0 },
                        { name: "entry_node_lookup_count", unit: "lookups", kind: Exact, target: 2 },
                        { name: "request_workspace_scan_count", unit: "scans", kind: Exact, target: 0 }
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
                description: "Resident retrieval, parser grounding, and blocking topology closure share the Runtime daemon resource supervisor while retaining intermediate-result and cached-topology memory reservations.",
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
                    target_rationale: "Three resident stages use one bounded queue and declare input work bytes separately from retained memory; blocking topology owns its CPU and memory permits until its real lifecycle terminal.",
                    warmup_iterations: 16,
                    measure_iterations: 128,
                    metrics: [
                        { name: "queue_wait_micros", unit: "microseconds", kind: Maximum, target: 5000 },
                        { name: "queue_capacity", unit: "slots", kind: Exact, target: 1 },
                        { name: "admitted_cpu", unit: "lanes", kind: Exact, target: 1 },
                        { name: "admitted_work_bytes", unit: "bytes", kind: Exact, target: 1_048_576 },
                        { name: "peak_admitted_memory_bytes", unit: "bytes", kind: Exact, target: 2_097_152 },
                        { name: "completed_stage_count", unit: "count", kind: Exact, target: 3 },
                        { name: "runtime_owned_blocking_stage_count", unit: "count", kind: Exact, target: 1 }
                    ]
                }
            ),
            crate::asp_rust_project_harness_scenario!(
                name: RUNTIME_RESIDENT_GREP_SEMANTICS_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Resident GREP qualifies Unicode, boundary, glob, CRLF, multiline, fixed-string, zero-width, and post-verification limit semantics from one versioned case catalog.",
                fixture_root: "crates/agent-semantic-runtime-server/tests/unit/scenarios/runtime_resident_grep_semantics",
                tags: ["search", "runtime", "resident-grep", "rg-differential", "semantics"],
                commands: [
                    {
                        label: "resident-zero-process-matrix",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-runtime-server",
                            "runtime_resident_grep::tests::resident_grep_semantics_scenario_covers_v1_matrix_without_external_processes",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "explicit-rg-differential-qualification",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-runtime-server",
                            "runtime_resident_grep::tests::admitted_grep_matches_rg_reference_corpus",
                            "--",
                            "--ignored",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: QUERY_CALLABLE_SKELETON_COMPACT_PRESENTATION_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Human Query dispatches stable V1 source and callable-skeleton projections to distinct renderers; compact skeleton output omits wire authority copies while machine JSON remains complete.",
                fixture_root: "crates/agent-semantic-client/tests/unit/scenarios/query_callable_skeleton_compact_presentation",
                tags: ["search", "query", "projection", "callable-skeleton", "result-quality"],
                commands: [
                    {
                        label: "compact-human-presentation",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client",
                            "--test",
                            "query_playbook",
                            "projection_presentation::query_callable_skeleton_uses_compact_text_babel_presentation",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "mixed-projection-rejection",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-client",
                            "--test",
                            "query_playbook",
                            "projection_presentation::query_human_presentation_rejects_mixed_projection_receipt",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
            ),
            crate::asp_rust_project_harness_scenario!(
                name: SEARCH_RESULT_OWNER_SUPPORT_INDEX_SCENARIO_ID,
                package: ASP_SEARCH_SCENARIO_PACKAGE_NAME,
                description: "Search fan-in builds one request-local owner support index, preserves Agent-authored support order, and removes syntax owners lacking acquisition support before ranking and Top-30 projection.",
                fixture_root: "crates/agent-semantic-search/tests/unit/scenarios/search_result_owner_support_index",
                tags: ["search", "fan-in", "owner-index", "result-quality", "complexity"],
                commands: [
                    {
                        label: "unrelated-owner-rejection",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-search",
                            "workspace_playbook_result_tests::fan_in_rejects_unrelated_syntax_owner_before_ranking_and_limit",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "graph-cannot-promote-unbacked-owner",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-search",
                            "workspace_playbook_result_tests::graph_cannot_promote_an_owner_without_acquisition_support",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "indexed-work-metrics",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-search",
                            "workspace_playbook_result_tests::fan_in_owner_support_index_scenario_records_work_reduction",
                            "--",
                            "--exact",
                            "--nocapture",
                        ]
                    },
                ],
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
