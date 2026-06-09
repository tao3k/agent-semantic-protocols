"""Typed graph frontier ranking algorithm."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .diversity import (
    normalize_kind_budgets,
    rank_nodes,
    selector_for_node,
)
from .model import GraphProfile, GraphResult, TypedGraph
from .profiles import resolve_profile
from .query_token_balance import query_tokens_for_seed_nodes
from .ranking_build import build_graph_result, rank_fingerprint
from .ranking_score import collect_scores, seed_ids as resolve_seed_ids
from .read_loop_second_pass import graph_turbo_apply_read_loop_second_pass
from .selector import (
    graph_turbo_node_range,
    graph_turbo_parse_selector,
    graph_turbo_ranges_adjacent,
)


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
    (
        scores,
        best_depth,
        selected_edges,
        graph_cache,
        receipt_adjustments,
        pagerank,
    ) = collect_scores(
        graph,
        selected_profile,
        seed_ids,
        fingerprint=fingerprint,
        cache_enabled=cache_enabled,
    )
    read_memory_selectors = frozenset(
        selector
        for selector in seen_selectors
        if isinstance(selector, str) and selector
    )
    suppressed_selectors = _read_memory_suppressed_selectors(
        graph,
        scores,
        read_memory_selectors,
        max_gap_lines=window_merge_max_gap_lines,
    )
    ranked_candidates = rank_nodes(
        graph,
        scores,
        best_depth,
        _candidate_limit(limit),
        normalized_kind_budgets,
        suppressed_selectors,
        query_tokens_for_seed_nodes(graph, seed_ids),
        coverage_limit=limit,
    )
    ranked, read_loop_second_pass = graph_turbo_apply_read_loop_second_pass(
        selected_profile,
        ranked_candidates,
        limit=limit,
        max_gap_lines=window_merge_max_gap_lines,
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
        pagerank,
        ranked,
        len(suppressed_selectors),
        read_loop_second_pass,
    )


def _candidate_limit(limit: int) -> int:
    return max(limit, limit + min(max(limit, 3), 8))


def _read_memory_suppressed_selectors(
    graph: TypedGraph,
    node_ids: Iterable[str],
    read_memory_selectors: frozenset[str],
    *,
    max_gap_lines: int,
) -> frozenset[str]:
    if not read_memory_selectors:
        return frozenset()
    read_memory_ranges = tuple(
        parsed
        for selector in read_memory_selectors
        if (parsed := graph_turbo_parse_selector(selector)) is not None
    )
    suppressed: set[str] = set()
    for node_id in node_ids:
        node = graph.nodes[node_id]
        selector = selector_for_node(node)
        if selector is None:
            continue
        if selector in read_memory_selectors:
            suppressed.add(selector)
            continue
        node_range = graph_turbo_node_range(node)
        if node_range is None:
            continue
        if any(
            graph_turbo_ranges_adjacent(
                node_range, memory_range, max_gap_lines=max_gap_lines
            )
            for memory_range in read_memory_ranges
        ):
            suppressed.add(selector)
    return frozenset(suppressed)
