"""Graph-turbo trace, metrics, and avoid-action artifacts."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass

from .constants import compact_avoid_actions_for_profile
from .evidence import algorithm_metrics, algorithm_trace
from .model import (
    AlgorithmMetrics,
    AlgorithmTraceStep,
    GraphCache,
    GraphProfile,
    Node,
    ReadLoopGuard,
    TypedGraph,
)
from .pagerank import GraphTurboPprResult
from .ranking_projection import RankedProjection
from .read_loop_second_pass import GraphTurboReadLoopSecondPass


@dataclass(frozen=True, slots=True)
class TraceProjection:
    trace: tuple[AlgorithmTraceStep, ...]
    metrics: AlgorithmMetrics


def build_trace_projection(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    *,
    graph_cache: GraphCache,
    ranked: tuple[Node, ...],
    projection: RankedProjection,
    best_depth: Mapping[str, int],
    path_count: int,
    path_backend: str,
    path_fallback_count: int,
    path_pair_count: int,
    path_candidate_count: int,
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
    pagerank: GraphTurboPprResult,
    query_adjustment_policy: Mapping[str, bool],
    query_adjustment_metrics: Mapping[str, int | float],
    runtime_cache_statuses: Mapping[str, str],
    read_loop_second_pass: GraphTurboReadLoopSecondPass,
) -> TraceProjection:
    return TraceProjection(
        trace=_algorithm_trace(
            graph,
            selected_profile,
            graph_cache,
            ranked,
            projection,
            best_depth,
            path_count,
            path_backend,
            path_fallback_count,
            path_pair_count,
            path_candidate_count,
            read_memory_suppressed_count,
            receipt_boost_count,
            receipt_penalty_count,
            pagerank,
            query_adjustment_policy,
            query_adjustment_metrics,
            read_loop_second_pass,
        ),
        metrics=_algorithm_metrics(
            graph,
            selected_profile,
            graph_cache,
            ranked,
            projection,
            best_depth,
            path_count,
            path_backend,
            path_fallback_count,
            path_pair_count,
            path_candidate_count,
            read_memory_suppressed_count,
            receipt_boost_count,
            receipt_penalty_count,
            pagerank,
            query_adjustment_metrics,
            runtime_cache_statuses,
            read_loop_second_pass,
        ),
    )


def compact_avoid_actions(
    selected_profile: GraphProfile,
    read_loop_guard: ReadLoopGuard,
    *,
    read_memory_suppressed_count: int,
    receipt_penalty_count: int,
) -> tuple[str, ...]:
    seen_selector = (
        ("seen-selector",)
        if read_memory_suppressed_count or receipt_penalty_count
        else ()
    )
    return _unique(
        (
            *compact_avoid_actions_for_profile(selected_profile.name),
            *read_loop_guard.avoid,
            *seen_selector,
        )
    )


def _algorithm_trace(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    graph_cache: GraphCache,
    ranked: tuple[Node, ...],
    projection: RankedProjection,
    best_depth: Mapping[str, int],
    path_count: int,
    path_backend: str,
    path_fallback_count: int,
    path_pair_count: int,
    path_candidate_count: int,
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
    pagerank: GraphTurboPprResult,
    query_adjustment_policy: Mapping[str, bool],
    query_adjustment_metrics: Mapping[str, int | float],
    read_loop_second_pass: GraphTurboReadLoopSecondPass,
) -> tuple[AlgorithmTraceStep, ...]:
    relation_channel_count = _selected_relation_channel_count(
        projection, selected_profile
    )
    return algorithm_trace(
        graph,
        selected_profile,
        graph_cache.status,
        reachable_count=len(best_depth),
        ranked_count=len(ranked),
        path_count=path_count,
        path_backend=path_backend,
        path_fallback_count=path_fallback_count,
        path_pair_count=path_pair_count,
        path_candidate_count=path_candidate_count,
        merged_window_count=len(projection.merged_windows),
        read_loop_guard=projection.read_loop_guard,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
        relation_channel_count=relation_channel_count,
        ppr_iterations=pagerank.iterations,
        ppr_residual=pagerank.residual,
        ppr_dangling_mass_last=pagerank.dangling_mass_last,
        ppr_mass_sum=pagerank.mass_sum,
        query_adjustment_policy=query_adjustment_policy,
        query_adjustment_metrics=query_adjustment_metrics,
        read_loop_second_pass=read_loop_second_pass,
    )


def _algorithm_metrics(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    graph_cache: GraphCache,
    ranked: tuple[Node, ...],
    projection: RankedProjection,
    best_depth: Mapping[str, int],
    path_count: int,
    path_backend: str,
    path_fallback_count: int,
    path_pair_count: int,
    path_candidate_count: int,
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
    pagerank: GraphTurboPprResult,
    query_adjustment_metrics: Mapping[str, int | float],
    runtime_cache_statuses: Mapping[str, str],
    read_loop_second_pass: GraphTurboReadLoopSecondPass,
) -> AlgorithmMetrics:
    relation_channel_count = _selected_relation_channel_count(
        projection, selected_profile
    )
    return algorithm_metrics(
        graph,
        selected_edge_count=len(projection.edges),
        reachable_node_count=len(best_depth),
        ranked_node_count=len(ranked),
        path_count=path_count,
        path_backend=path_backend,
        path_fallback_count=path_fallback_count,
        path_pair_count=path_pair_count,
        path_candidate_count=path_candidate_count,
        merged_window_count=len(projection.merged_windows),
        cache_status=graph_cache.status,
        depth_cache_status=runtime_cache_statuses.get("depthCacheStatus", "unknown"),
        ppr_cache_status=runtime_cache_statuses.get("pprCacheStatus", "unknown"),
        reachable_edges_cache_status=runtime_cache_statuses.get(
            "reachableEdgesCacheStatus",
            "unknown",
        ),
        read_loop_guard=projection.read_loop_guard,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
        relation_channel_count=relation_channel_count,
        ppr_iterations=pagerank.iterations,
        ppr_residual=pagerank.residual,
        ppr_dangling_mass_last=pagerank.dangling_mass_last,
        ppr_mass_sum=pagerank.mass_sum,
        read_loop_second_pass_suppressed_count=(read_loop_second_pass.suppressed_count),
        read_loop_duplicate_selector_suppressed_count=(
            read_loop_second_pass.duplicate_selector_suppressed_count
        ),
        read_loop_adjacent_range_merged_count=(
            read_loop_second_pass.adjacent_range_merged_count
        ),
        read_loop_same_owner_suppressed_count=(
            read_loop_second_pass.same_owner_suppressed_count
        ),
        query_adjustment_metrics=query_adjustment_metrics,
    )


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    unique_values: list[str] = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        unique_values.append(value)
    return tuple(unique_values)


def _selected_relation_channel_count(
    projection: RankedProjection, selected_profile: GraphProfile | None
) -> int:
    if selected_profile is None:
        return 0
    for matrix in projection.matrices:
        if matrix.profile == selected_profile.name:
            return len(matrix.relation_channels)
    return 0
