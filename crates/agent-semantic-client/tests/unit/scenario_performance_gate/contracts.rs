// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::shared::SharedBenchmarkToml;

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

pub(crate) fn assert_search_query_budget_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-query-budget")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
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

pub(super) fn assert_graph_evidence_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-evidence-projection")
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
const _: fn(&SharedBenchmarkToml) = assert_search_query_budget_benchmark_contract;
