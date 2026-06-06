"""Typed graph frontier ranking algorithm."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .backend import multi_source_hop_lengths, reachable_edges, typed_personalized_pagerank
from .cache import cached_sparse_backend, packet_fingerprint
from .compatibility import profile_compatibility
from .constants import COMPACT_AVOID_ACTIONS, COMPACT_OMISSIONS
from .diversity import normalize_kind_budgets, rank_nodes
from .evidence import algorithm_metrics, algorithm_trace, rank_explanations
from .model import (
    Edge,
    FrontierEntry,
    GraphCache,
    GraphProfile,
    GraphResult,
    Node,
    TypedGraph,
)
from .paths import flow_lite, source_sink_frontier, typed_paths
from .policy import node_kind_bonus
from .profiles import DEFAULT_PROFILES, frontier_action, resolve_profile
from .windows import merge_ranked_windows


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
) -> GraphResult:
    selected_profile = resolve_profile(profile)
    seed_ids = _seed_ids(graph, seeds)
    normalized_kind_budgets = normalize_kind_budgets(kind_budgets)
    fingerprint = _rank_fingerprint(
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
    scores, best_depth, selected_edges, graph_cache = _collect_scores(
        graph,
        selected_profile,
        seed_ids,
        fingerprint=fingerprint,
        cache_enabled=cache_enabled,
    )
    ranked = rank_nodes(graph, scores, best_depth, limit, normalized_kind_budgets)
    return _build_graph_result(
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
        ranked,
    )


def _rank_fingerprint(
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


def _build_graph_result(
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
    ranked: tuple[Node, ...],
) -> GraphResult:
    frontier = _frontier_entries(selected_profile, ranked, scores)
    edges = _ranked_edges(selected_edges.values(), ranked)
    merged_windows = merge_ranked_windows(
        ranked,
        enabled=window_merge_enabled,
        max_gap_lines=window_merge_max_gap_lines,
    )
    compatibility = profile_compatibility(edges, ranked)
    ranked_ids = tuple(node.id for node in ranked)
    source_sink = source_sink_frontier(graph, selected_profile, seed_ids, ranked_ids)
    paths = typed_paths(
        graph,
        selected_profile,
        source_sink,
        scores,
        path_budget=path_budget,
        max_hops=path_max_hops,
    )
    flow = flow_lite(paths)
    explanations = rank_explanations(
        ranked, selected_profile, scores, best_depth, seed_ids, normalized_kind_budgets
    )
    trace = algorithm_trace(
        graph,
        selected_profile,
        graph_cache.status,
        reachable_count=len(best_depth),
        ranked_count=len(ranked),
        path_count=len(paths),
        merged_window_count=len(merged_windows),
    )
    metrics = algorithm_metrics(
        graph,
        selected_edge_count=len(edges),
        reachable_node_count=len(best_depth),
        ranked_node_count=len(ranked),
        path_count=len(paths),
        merged_window_count=len(merged_windows),
        cache_status=graph_cache.status,
    )
    return GraphResult(
        selected_profile,
        seed_ids,
        ranked,
        frontier,
        scores,
        edges,
        limit,
        normalized_kind_budgets,
        merged_windows,
        compatibility,
        source_sink,
        paths,
        flow,
        fingerprint,
        graph_cache,
        trace,
        explanations,
        metrics,
        tuple(DEFAULT_PROFILES),
        COMPACT_OMISSIONS,
        COMPACT_AVOID_ACTIONS,
    )


def _seed_ids(graph: TypedGraph, seeds: Iterable[str]) -> tuple[str, ...]:
    seed_ids = _unique(seed for seed in seeds if seed in graph.nodes)
    if not seed_ids:
        seed_ids = _unique(
            node.id for node in graph.nodes.values() if node.kind in {"query", "owner"}
        )
    if not seed_ids:
        seed_ids = tuple(graph.nodes.keys())
    return seed_ids


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    unique_values: list[str] = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        unique_values.append(value)
    return tuple(unique_values)


def _collect_scores(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: Iterable[str],
    *,
    fingerprint: str,
    cache_enabled: bool,
) -> tuple[dict[str, float], dict[str, int], dict[tuple[str, str, str], Edge], GraphCache]:
    backend, graph_cache = cached_sparse_backend(
        graph, profile, fingerprint, enabled=cache_enabled
    )
    best_depth = multi_source_hop_lengths(backend, seed_ids, profile.max_depth)
    pagerank = typed_personalized_pagerank(backend, seed_ids)
    scores = _score_nodes(graph, profile, best_depth, pagerank)
    selected_edges = reachable_edges(backend, best_depth)
    return scores, best_depth, selected_edges, graph_cache


def _score_nodes(
    graph: TypedGraph,
    profile: GraphProfile,
    best_depth: Mapping[str, int],
    pagerank: Mapping[str, float],
) -> dict[str, float]:
    max_pagerank = max(
        (pagerank.get(node_id, 0.0) for node_id in best_depth),
        default=0.0,
    )
    return {
        node_id: graph.nodes[node_id].weight
        + (float(profile.max_depth - depth + 1) * 0.2)
        + (
            pagerank.get(node_id, 0.0) / max_pagerank
            if max_pagerank > 0.0
            else 0.0
        )
        + node_kind_bonus(profile.name, graph.nodes[node_id].kind)
        for node_id, depth in best_depth.items()
    }


def _frontier_entries(
    profile: GraphProfile, ranked: Iterable[Node], scores: Mapping[str, float]
) -> tuple[FrontierEntry, ...]:
    entries: list[FrontierEntry] = []
    for node in ranked:
        action = frontier_action(profile, node)
        if action is None:
            continue
        entries.append(FrontierEntry(node, action, scores[node.id]))
    return tuple(entries)


def _ranked_edges(edges: Iterable[Edge], ranked: Iterable[Node]) -> tuple[Edge, ...]:
    ranked_ids = {node.id for node in ranked}
    return tuple(
        edge
        for edge in edges
        if edge.source in ranked_ids and edge.target in ranked_ids
    )
