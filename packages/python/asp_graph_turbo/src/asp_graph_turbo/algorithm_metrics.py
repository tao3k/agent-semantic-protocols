"""Algorithm metrics projection for graph turbo responses."""

from __future__ import annotations

from collections.abc import Mapping

from .model import AlgorithmMetrics, ReadLoopGuard, TypedGraph


def algorithm_metrics(
    graph: TypedGraph,
    *,
    selected_edge_count: int,
    reachable_node_count: int,
    ranked_node_count: int,
    path_count: int,
    path_backend: str = "python-bfs-small",
    path_fallback_count: int = 0,
    path_pair_count: int = 0,
    path_candidate_count: int = 0,
    merged_window_count: int,
    cache_status: str,
    depth_cache_status: str = "unknown",
    ppr_cache_status: str = "unknown",
    reachable_edges_cache_status: str = "unknown",
    read_loop_guard: ReadLoopGuard | None = None,
    read_memory_suppressed_count: int = 0,
    receipt_boost_count: int = 0,
    receipt_penalty_count: int = 0,
    relation_channel_count: int = 0,
    ppr_iterations: int = 0,
    ppr_residual: float = 0.0,
    ppr_dangling_mass_last: float = 0.0,
    ppr_mass_sum: float = 0.0,
    read_loop_second_pass_suppressed_count: int = 0,
    read_loop_duplicate_selector_suppressed_count: int = 0,
    read_loop_adjacent_range_merged_count: int = 0,
    read_loop_same_owner_suppressed_count: int = 0,
    query_adjustment_metrics: Mapping[str, int | float] | None = None,
) -> AlgorithmMetrics:
    guard = read_loop_guard or ReadLoopGuard(0, 0, 0, 0, ())
    query_metrics = query_adjustment_metrics or {}
    return AlgorithmMetrics(
        node_count=len(graph.nodes),
        edge_count=len(graph.edges),
        selected_edge_count=selected_edge_count,
        reachable_node_count=reachable_node_count,
        ranked_node_count=ranked_node_count,
        path_count=path_count,
        path_backend=path_backend,
        path_fallback_count=path_fallback_count,
        path_pair_count=path_pair_count,
        path_candidate_count=path_candidate_count,
        merged_window_count=merged_window_count,
        cache_status=cache_status,
        depth_cache_status=depth_cache_status,
        ppr_cache_status=ppr_cache_status,
        reachable_edges_cache_status=reachable_edges_cache_status,
        read_loop_direct_code_action_count=guard.direct_code_action_count,
        read_loop_duplicate_selector_count=guard.duplicate_selector_count,
        read_loop_adjacent_range_window_count=guard.adjacent_range_window_count,
        read_loop_same_owner_scan_count=guard.same_owner_scan_count,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
        relation_channel_count=relation_channel_count,
        ppr_iterations=ppr_iterations,
        ppr_residual=ppr_residual,
        ppr_dangling_mass_last=ppr_dangling_mass_last,
        ppr_mass_sum=ppr_mass_sum,
        read_loop_second_pass_suppressed_count=read_loop_second_pass_suppressed_count,
        read_loop_duplicate_selector_suppressed_count=(
            read_loop_duplicate_selector_suppressed_count
        ),
        read_loop_adjacent_range_merged_count=read_loop_adjacent_range_merged_count,
        read_loop_same_owner_suppressed_count=read_loop_same_owner_suppressed_count,
        **_query_metric_kwargs(query_metrics),
    )


def _query_metric_kwargs(
    query_metrics: Mapping[str, int | float],
) -> dict[str, int | float]:
    return {
        "query_seed_prior_count": _int_metric(query_metrics, "querySeedPriorCount"),
        "query_seed_prior_mass": _float_metric(query_metrics, "querySeedPriorMass"),
        "query_package_cohesion_count": _int_metric(
            query_metrics, "queryPackageCohesionCount"
        ),
        "query_package_drift_penalty_count": _int_metric(
            query_metrics, "queryPackageDriftPenaltyCount"
        ),
        "query_package_cohesion_delta": _float_metric(
            query_metrics, "queryPackageCohesionDelta"
        ),
        "query_clause_coverage_count": _int_metric(
            query_metrics, "queryClauseCoverageCount"
        ),
        "query_clause_coverage_delta": _float_metric(
            query_metrics, "queryClauseCoverageDelta"
        ),
        "query_local_evidence_boost_count": _int_metric(
            query_metrics, "queryLocalEvidenceBoostCount"
        ),
        "query_local_evidence_penalty_count": _int_metric(
            query_metrics, "queryLocalEvidencePenaltyCount"
        ),
        "query_local_evidence_delta": _float_metric(
            query_metrics, "queryLocalEvidenceDelta"
        ),
    }


def _int_metric(metrics: Mapping[str, int | float], name: str) -> int:
    value = metrics.get(name)
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, float):
        return int(value)
    return 0


def _float_metric(metrics: Mapping[str, int | float], name: str) -> float:
    value = metrics.get(name)
    return float(value) if isinstance(value, int | float) else 0.0
