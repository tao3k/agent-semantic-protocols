use super::shared::SharedBenchmarkToml;

pub(crate) fn assert_dynamic_overlay_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("dynamic-overlay"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_turso_overlay_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("turso-overlay"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_evidence_graph_rank_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("evidence-graph-rank")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_search_candidate_contract_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-candidate-contract")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_query_wrapper_source_index_bridge_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-source-index-bridge")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_query_wrapper_render_hint_projection_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-render-hint-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_search_owner_source_index_trace_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
    expected_fallback_reason: &str,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-owner-source-index-trace")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(1024));
    assert_eq!(
        benchmark.fallback_reason.as_deref(),
        Some(expected_fallback_reason)
    );
}

pub(crate) fn assert_query_wrapper_clause_normalization_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-clause-normalization")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_search_query_budget_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-query-budget")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_generated_candidate_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-generated-candidate")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(crate) fn assert_provider_candidate_annotations_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("provider-candidate-annotations")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_node_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-node-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_candidate_projection_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-candidate-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_topology_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-topology-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_owner_rank_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("graph-owner-rank"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_query_owner_seed_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-query-owner-seed")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_seed_decision_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-seed-decision")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_graph_evidence_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-evidence-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_search_pipe_package_cohesion_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-package-cohesion")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_search_pipe_query_pack_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-query-pack")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_search_pipe_quality_decision_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-quality-decision")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_search_pipe_evidence_classifier_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-evidence-classifier")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_runtime_owner_items_receipt_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("owner-items-runtime")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(1));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_rust_owner_items_minimal_ast_cut_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("dynamic-owner-items")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

pub(super) fn assert_runtime_timeout_policy_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("runtime-timeout-policy")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(1024));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}
