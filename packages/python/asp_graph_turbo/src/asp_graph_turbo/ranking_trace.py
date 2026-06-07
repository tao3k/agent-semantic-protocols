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
from .ranking_projection import RankedProjection


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
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
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
            read_memory_suppressed_count,
            receipt_boost_count,
            receipt_penalty_count,
        ),
        metrics=_algorithm_metrics(
            graph,
            graph_cache,
            ranked,
            projection,
            best_depth,
            path_count,
            read_memory_suppressed_count,
            receipt_boost_count,
            receipt_penalty_count,
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
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
) -> tuple[AlgorithmTraceStep, ...]:
    return algorithm_trace(
        graph,
        selected_profile,
        graph_cache.status,
        reachable_count=len(best_depth),
        ranked_count=len(ranked),
        path_count=path_count,
        merged_window_count=len(projection.merged_windows),
        read_loop_guard=projection.read_loop_guard,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
    )


def _algorithm_metrics(
    graph: TypedGraph,
    graph_cache: GraphCache,
    ranked: tuple[Node, ...],
    projection: RankedProjection,
    best_depth: Mapping[str, int],
    path_count: int,
    read_memory_suppressed_count: int,
    receipt_boost_count: int,
    receipt_penalty_count: int,
) -> AlgorithmMetrics:
    return algorithm_metrics(
        graph,
        selected_edge_count=len(projection.edges),
        reachable_node_count=len(best_depth),
        ranked_node_count=len(ranked),
        path_count=path_count,
        merged_window_count=len(projection.merged_windows),
        cache_status=graph_cache.status,
        read_loop_guard=projection.read_loop_guard,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
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
