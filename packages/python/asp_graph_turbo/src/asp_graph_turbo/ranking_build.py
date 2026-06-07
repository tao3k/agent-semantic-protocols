"""Build graph-turbo ranking result objects."""

from __future__ import annotations

from collections.abc import Mapping

from .cache import packet_fingerprint
from .constants import compact_omissions_for_profile
from .evidence import rank_explanations
from .model import (
    Edge,
    GraphCache,
    GraphProfile,
    GraphResult,
    Node,
    ReceiptAdjustment,
    TypedGraph,
)
from .profiles import DEFAULT_PROFILES
from .ranking_projection import (
    build_path_projection,
    build_ranked_projection,
)
from .receipt import receipt_adjustment_counts, receipt_reasons_by_node
from .ranking_trace import build_trace_projection, compact_avoid_actions


def rank_fingerprint(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: tuple[str, ...],
    budget: int,
    kind_budgets: Mapping[str, int],
    path_budget: int,
    path_max_hops: int,
    window_merge_enabled: bool,
    window_merge_max_gap_lines: int,
) -> str:
    return packet_fingerprint(
        graph,
        profile,
        seed_ids=seed_ids,
        budget=budget,
        kind_budgets=kind_budgets,
        path_budget=path_budget,
        path_max_hops=path_max_hops,
        window_merge_enabled=window_merge_enabled,
        window_merge_max_gap_lines=window_merge_max_gap_lines,
    )


def build_graph_result(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    seed_ids: tuple[str, ...],
    normalized_kind_budgets: Mapping[str, int],
    limit: int,
    path_budget: int,
    path_max_hops: int,
    window_merge_enabled: bool,
    window_merge_max_gap_lines: int,
    fingerprint: str,
    scores: Mapping[str, float],
    best_depth: Mapping[str, int],
    selected_edges: Mapping[tuple[str, str, str], Edge],
    graph_cache: GraphCache,
    receipt_adjustments: tuple[ReceiptAdjustment, ...],
    ranked: tuple[Node, ...],
    read_memory_suppressed_count: int = 0,
) -> GraphResult:
    ranked_projection = build_ranked_projection(
        graph=graph,
        selected_profile=selected_profile,
        scores=scores,
        selected_edges=selected_edges.values(),
        best_depth=best_depth,
        ranked=ranked,
        window_merge_enabled=window_merge_enabled,
        window_merge_max_gap_lines=window_merge_max_gap_lines,
    )
    path_projection = build_path_projection(
        graph,
        selected_profile,
        seed_ids,
        ranked_projection.ranked_ids,
        scores,
        path_budget=path_budget,
        path_max_hops=path_max_hops,
    )
    receipt_counts = receipt_adjustment_counts(receipt_adjustments)
    explanations = rank_explanations(
        ranked,
        selected_profile,
        scores,
        best_depth,
        seed_ids,
        normalized_kind_budgets,
        receipt_reasons_by_node(receipt_adjustments),
    )
    trace_projection = build_trace_projection(
        graph,
        selected_profile,
        graph_cache=graph_cache,
        projection=ranked_projection,
        best_depth=best_depth,
        ranked=ranked,
        path_count=len(path_projection.paths),
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_counts[0],
        receipt_penalty_count=receipt_counts[1],
    )
    return GraphResult(
        selected_profile,
        seed_ids,
        ranked,
        ranked_projection.frontier,
        scores,
        ranked_projection.edges,
        limit,
        normalized_kind_budgets,
        ranked_projection.merged_windows,
        ranked_projection.compatibility,
        ranked_projection.matrices,
        path_projection.source_sink,
        path_projection.paths,
        path_projection.flow,
        fingerprint,
        graph_cache,
        trace_projection.trace,
        explanations,
        receipt_adjustments,
        trace_projection.metrics,
        tuple(DEFAULT_PROFILES),
        compact_omissions_for_profile(selected_profile.name),
        compact_avoid_actions(
            selected_profile,
            ranked_projection.read_loop_guard,
            read_memory_suppressed_count=read_memory_suppressed_count,
            receipt_penalty_count=receipt_counts[1],
        ),
    )
