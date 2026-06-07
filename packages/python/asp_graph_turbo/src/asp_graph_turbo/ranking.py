"""Typed graph frontier ranking algorithm."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .diversity import normalize_kind_budgets, rank_nodes, selector_for_node
from .model import GraphProfile, GraphResult, TypedGraph
from .profiles import resolve_profile
from .ranking_build import build_graph_result, rank_fingerprint
from .ranking_score import collect_scores, seed_ids as resolve_seed_ids


def rank_frontier(
    graph: TypedGraph,
    *,
    profile: str | GraphProfile = "owner-query",
    seeds: Iterable[str] = (),
    limit: int = 8,
    kind_budgets: Mapping[str, int] | None = None,
    window_merge_enabled: bool = True,
    window_merge_max_gap_lines: int = 8,
    path_budget: int = 4,
    path_max_hops: int = 4,
    cache_enabled: bool = True,
    seen_selectors: Iterable[str] = (),
) -> GraphResult:
    selected_profile = resolve_profile(profile)
    seed_ids = resolve_seed_ids(graph, seeds)
    normalized_kind_budgets = normalize_kind_budgets(kind_budgets)
    fingerprint = rank_fingerprint(
        graph,
        selected_profile,
        seed_ids,
        limit,
        normalized_kind_budgets,
        path_budget,
        path_max_hops,
        window_merge_enabled,
        window_merge_max_gap_lines,
    )
    scores, best_depth, selected_edges, graph_cache, receipt_adjustments = collect_scores(
        graph,
        selected_profile,
        seed_ids,
        fingerprint=fingerprint,
        cache_enabled=cache_enabled,
    )
    read_memory_selectors = frozenset(
        selector for selector in seen_selectors if isinstance(selector, str) and selector
    )
    read_memory_suppressed_count = sum(
        1
        for node_id in scores
        if selector_for_node(graph.nodes[node_id]) in read_memory_selectors
    )
    ranked = rank_nodes(
        graph,
        scores,
        best_depth,
        limit,
        normalized_kind_budgets,
        read_memory_selectors,
    )
    return build_graph_result(
        graph,
        selected_profile,
        seed_ids,
        normalized_kind_budgets,
        limit,
        path_budget,
        path_max_hops,
        window_merge_enabled,
        window_merge_max_gap_lines,
        fingerprint,
        scores,
        best_depth,
        selected_edges,
        graph_cache,
        receipt_adjustments,
        ranked,
        read_memory_suppressed_count,
    )
