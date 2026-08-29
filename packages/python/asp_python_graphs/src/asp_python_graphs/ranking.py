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
from .query_adjustments import normalize_query_adjustment_policy
from .query_token_priority import (
    prioritized_query_tokens,
    query_token_balance_weights,
)
from .query_token_balance import (
    query_tokens_for_seed_nodes,
)
from .ranking_build import build_graph_result, rank_fingerprint
from .ranking_score import collect_scores, seed_ids as resolve_seed_ids
from .read_loop_second_pass import graph_turbo_apply_read_loop_second_pass
from asp_memory_engine.graph_turbo_memory import read_memory_projection


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
    query_clauses: Iterable[str] = (),
    query_adjustment_policy: Mapping[str, object] | None = None,
) -> GraphResult:
    selected_profile = resolve_profile(profile)
    seed_ids = resolve_seed_ids(graph, seeds)
    normalized_query_clauses = _normalized_query_clauses(query_clauses)
    normalized_query_adjustment_policy = normalize_query_adjustment_policy(
        query_adjustment_policy
    )
    normalized_kind_budgets = normalize_kind_budgets(kind_budgets)
    fingerprint = rank_fingerprint(
        graph,
        selected_profile,
        seed_ids,
        normalized_query_clauses,
        normalized_query_adjustment_policy,
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
        query_adjustments,
        runtime_cache_statuses,
    ) = collect_scores(
        graph,
        selected_profile,
        seed_ids,
        cache_enabled=cache_enabled,
        query_clauses=normalized_query_clauses,
        query_adjustment_policy=normalized_query_adjustment_policy,
    )
    read_memory = read_memory_projection(
        [selector_for_node(graph.nodes[node_id]) for node_id in scores],
        tuple(seen_selectors),
        max_gap_lines=window_merge_max_gap_lines,
    )
    query_tokens = prioritized_query_tokens(
        query_tokens_for_seed_nodes(graph, seed_ids),
        normalized_query_clauses,
    )
    ranked_candidates = rank_nodes(
        graph,
        scores,
        best_depth,
        _candidate_limit(limit),
        normalized_kind_budgets,
        frozenset(read_memory.suppressed_selectors),
        query_tokens,
        query_token_weights=query_token_balance_weights(
            query_tokens,
            normalized_query_clauses,
        ),
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
        query_adjustments,
        runtime_cache_statuses,
        normalized_query_adjustment_policy,
        ranked,
        read_memory.seen_selectors,
        read_memory.suppressed_selectors,
        read_loop_second_pass,
    )


def _candidate_limit(limit: int) -> int:
    return max(limit, limit + min(max(limit, 3), 8))


def _normalized_query_clauses(query_clauses: Iterable[str]) -> tuple[str, ...]:
    clauses: list[str] = []
    for clause in query_clauses:
        if not isinstance(clause, str):
            continue
        normalized = " ".join(clause.split())
        if normalized:
            clauses.append(normalized)
    return tuple(clauses)
