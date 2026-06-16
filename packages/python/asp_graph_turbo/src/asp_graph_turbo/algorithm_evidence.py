"""Algorithm trace and metrics projection for graph turbo responses."""

from __future__ import annotations

from collections.abc import Mapping

from .model import (
    AlgorithmMetrics,
    AlgorithmTraceStep,
    GraphProfile,
    ReadLoopGuard,
    TypedGraph,
)
from .read_loop_second_pass import GraphTurboReadLoopSecondPass


def algorithm_trace(
    graph: TypedGraph,
    profile: GraphProfile,
    cache_status: str,
    *,
    reachable_count: int,
    ranked_count: int,
    path_count: int,
    path_backend: str = "python-bfs-small",
    path_fallback_count: int = 0,
    path_pair_count: int = 0,
    path_candidate_count: int = 0,
    merged_window_count: int,
    read_loop_guard: ReadLoopGuard | None = None,
    read_memory_suppressed_count: int = 0,
    receipt_boost_count: int = 0,
    receipt_penalty_count: int = 0,
    relation_channel_count: int = 0,
    ppr_iterations: int = 0,
    ppr_residual: float = 0.0,
    ppr_dangling_mass_last: float = 0.0,
    ppr_mass_sum: float = 0.0,
    query_adjustment_policy: Mapping[str, bool] | None = None,
    query_adjustment_metrics: Mapping[str, int | float] | None = None,
    read_loop_second_pass: GraphTurboReadLoopSecondPass = GraphTurboReadLoopSecondPass(),
) -> tuple[AlgorithmTraceStep, ...]:
    steps = [
        AlgorithmTraceStep(
            "packet-fingerprint",
            "sha256",
            {"nodeCount": len(graph.nodes), "edgeCount": len(graph.edges)},
        ),
        AlgorithmTraceStep("graph-cache", "memory", {"status": cache_status}),
        _profile_policy_step(profile),
        _typed_ppr_step(
            profile,
            reachable_count,
            relation_channel_count,
            ppr_iterations,
            ppr_residual,
            ppr_dangling_mass_last,
            ppr_mass_sum,
        ),
        AlgorithmTraceStep("diverse-rank", "python", {"rankedNodeCount": ranked_count}),
        _typed_paths_step(
            path_backend,
            path_count,
            path_fallback_count,
            path_pair_count,
            path_candidate_count,
        ),
        AlgorithmTraceStep(
            "window-merge",
            "python",
            {"mergedWindowCount": merged_window_count},
        ),
    ]
    _append_optional_trace_steps(
        steps,
        read_loop_guard,
        read_memory_suppressed_count,
        read_loop_second_pass,
        receipt_boost_count,
        receipt_penalty_count,
        query_adjustment_policy,
        query_adjustment_metrics,
    )
    return tuple(steps)


def _profile_policy_step(profile: GraphProfile) -> AlgorithmTraceStep:
    return AlgorithmTraceStep(
        "profile-policy",
        "python",
        {
            "profile": profile.name,
            "allowedRelationCount": len(profile.allowed_relations),
            "allowedTransitionCount": len(profile.allowed_transitions),
            "kindBonusCount": len(profile.kind_bonus),
        },
    )


def _typed_ppr_step(
    profile: GraphProfile,
    reachable_count: int,
    relation_channel_count: int,
    ppr_iterations: int,
    ppr_residual: float,
    ppr_dangling_mass_last: float,
    ppr_mass_sum: float,
) -> AlgorithmTraceStep:
    return AlgorithmTraceStep(
        "typed-ppr",
        "scipy-csr",
        {
            "profile": profile.name,
            "reachableNodeCount": reachable_count,
            "iterations": ppr_iterations,
            "residual": round(ppr_residual, 12),
            "massSum": round(ppr_mass_sum, 12),
            "danglingMass": round(ppr_dangling_mass_last, 12),
            "relationChannelCount": relation_channel_count,
        },
    )


def _typed_paths_step(
    path_backend: str,
    path_count: int,
    path_fallback_count: int,
    path_pair_count: int,
    path_candidate_count: int,
) -> AlgorithmTraceStep:
    return AlgorithmTraceStep(
        "typed-paths",
        path_backend,
        {
            "pathCount": path_count,
            "fallbackCount": path_fallback_count,
            "pairCount": path_pair_count,
            "candidateCount": path_candidate_count,
        },
    )


def _append_optional_trace_steps(
    steps: list[AlgorithmTraceStep],
    read_loop_guard: ReadLoopGuard | None,
    read_memory_suppressed_count: int,
    read_loop_second_pass: GraphTurboReadLoopSecondPass,
    receipt_boost_count: int,
    receipt_penalty_count: int,
    query_adjustment_policy: Mapping[str, bool] | None,
    query_adjustment_metrics: Mapping[str, int | float] | None,
) -> None:
    if read_loop_guard is not None:
        steps.append(_read_loop_guard_step(read_loop_guard))
    if read_memory_suppressed_count:
        steps.append(
            AlgorithmTraceStep(
                "read-memory-suppression",
                "python",
                {"suppressedSelectorCount": read_memory_suppressed_count},
            )
        )
    if read_loop_second_pass.suppressed_count:
        steps.append(_read_loop_second_pass_step(read_loop_second_pass))
    if receipt_boost_count or receipt_penalty_count:
        steps.append(
            AlgorithmTraceStep(
                "receipt-feedback",
                "python",
                {
                    "boostCount": receipt_boost_count,
                    "penaltyCount": receipt_penalty_count,
                },
            )
        )
    if _has_query_adjustment_metrics(query_adjustment_metrics):
        steps.append(
            AlgorithmTraceStep(
                "query-adjustments",
                "python",
                {
                    **dict(query_adjustment_metrics or {}),
                    "seedPriorEnabled": bool(
                        (query_adjustment_policy or {}).get("seedPrior", True)
                    ),
                    "packageCohesionEnabled": bool(
                        (query_adjustment_policy or {}).get("packageCohesion", True)
                    ),
                    "queryClauseCoverageEnabled": bool(
                        (query_adjustment_policy or {}).get(
                            "queryClauseCoverage", True
                        )
                    ),
                },
            )
        )


def _read_loop_guard_step(read_loop_guard: ReadLoopGuard) -> AlgorithmTraceStep:
    return AlgorithmTraceStep(
        "read-loop-guard",
        "python",
        {
            "directCodeActionCount": read_loop_guard.direct_code_action_count,
            "duplicateSelectorCount": read_loop_guard.duplicate_selector_count,
            "adjacentRangeWindowCount": read_loop_guard.adjacent_range_window_count,
            "sameOwnerScanCount": read_loop_guard.same_owner_scan_count,
        },
    )


def _read_loop_second_pass_step(
    read_loop_second_pass: GraphTurboReadLoopSecondPass,
) -> AlgorithmTraceStep:
    return AlgorithmTraceStep(
        "read-loop-second-pass",
        "python",
        {
            "candidateCount": read_loop_second_pass.candidate_count,
            "suppressedCount": read_loop_second_pass.suppressed_count,
            "duplicateSelectorSuppressedCount": (
                read_loop_second_pass.duplicate_selector_suppressed_count
            ),
            "adjacentRangeMergedCount": (
                read_loop_second_pass.adjacent_range_merged_count
            ),
            "sameOwnerSuppressedCount": (
                read_loop_second_pass.same_owner_suppressed_count
            ),
        },
    )


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
        read_loop_second_pass_suppressed_count=(read_loop_second_pass_suppressed_count),
        read_loop_duplicate_selector_suppressed_count=(
            read_loop_duplicate_selector_suppressed_count
        ),
        read_loop_adjacent_range_merged_count=read_loop_adjacent_range_merged_count,
        read_loop_same_owner_suppressed_count=read_loop_same_owner_suppressed_count,
        query_seed_prior_count=_int_metric(query_metrics, "querySeedPriorCount"),
        query_seed_prior_mass=_float_metric(query_metrics, "querySeedPriorMass"),
        query_package_cohesion_count=_int_metric(
            query_metrics, "queryPackageCohesionCount"
        ),
        query_package_drift_penalty_count=_int_metric(
            query_metrics, "queryPackageDriftPenaltyCount"
        ),
        query_package_cohesion_delta=_float_metric(
            query_metrics, "queryPackageCohesionDelta"
        ),
        query_clause_coverage_count=_int_metric(
            query_metrics, "queryClauseCoverageCount"
        ),
        query_clause_coverage_delta=_float_metric(
            query_metrics, "queryClauseCoverageDelta"
        ),
    )


def _has_query_adjustment_metrics(
    metrics: Mapping[str, int | float] | None,
) -> bool:
    if not metrics:
        return False
    return any(
        isinstance(value, int | float) and not isinstance(value, bool) and value != 0
        for value in metrics.values()
    )


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
