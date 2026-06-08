"""Score graph-turbo frontier nodes."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .backend import multi_source_hop_lengths, reachable_edges
from .cache import cached_sparse_backend
from .model import GraphCache, GraphProfile, OrientedEdge, ReceiptAdjustment, TypedGraph
from .pagerank import (
    GraphTurboPprResult,
    graph_turbo_typed_personalized_pagerank_result,
)
from .query_weights import query_node_match_bonus, query_token_weights
from .receipt import receipt_score_adjustments


def seed_ids(graph: TypedGraph, seeds: Iterable[str]) -> tuple[str, ...]:
    selected_seed_ids = _unique(seed for seed in seeds if seed in graph.nodes)
    if not selected_seed_ids:
        selected_seed_ids = _unique(
            node.id for node in graph.nodes.values() if node.kind in {"query", "owner"}
        )
    if not selected_seed_ids:
        selected_seed_ids = tuple(graph.nodes.keys())
    return selected_seed_ids


def collect_scores(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: Iterable[str],
    *,
    fingerprint: str,
    cache_enabled: bool,
) -> tuple[
    dict[str, float],
    dict[str, int],
    dict[tuple[str, str, str], OrientedEdge],
    GraphCache,
    tuple[ReceiptAdjustment, ...],
    GraphTurboPprResult,
]:
    backend, graph_cache = cached_sparse_backend(
        graph, profile, fingerprint, enabled=cache_enabled
    )
    best_depth = multi_source_hop_lengths(backend, seed_ids, profile.max_depth)
    pagerank = graph_turbo_typed_personalized_pagerank_result(backend, seed_ids)
    scores = score_nodes(graph, profile, seed_ids, best_depth, pagerank.scores)
    receipt_adjustments, receipt_facts = receipt_score_adjustments(graph)
    for node_id, score_delta in receipt_adjustments.items():
        if node_id in scores:
            scores[node_id] += score_delta
    selected_edges = reachable_edges(backend, best_depth)
    return scores, best_depth, selected_edges, graph_cache, receipt_facts, pagerank


def score_nodes(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: Iterable[str],
    best_depth: Mapping[str, int],
    pagerank: Mapping[str, float],
) -> dict[str, float]:
    max_pagerank = max(
        (pagerank.get(node_id, 0.0) for node_id in best_depth),
        default=0.0,
    )
    token_weights = query_token_weights(
        graph,
        profile_name=profile.name,
        seed_ids=seed_ids,
    )
    return {
        node_id: graph.nodes[node_id].weight
        + (float(profile.max_depth - depth + 1) * 0.2)
        + (pagerank.get(node_id, 0.0) / max_pagerank if max_pagerank > 0.0 else 0.0)
        + profile.kind_bonus.get(graph.nodes[node_id].kind, 0.0)
        + query_node_match_bonus(
            profile_name=profile.name,
            token_weights=token_weights,
            node=graph.nodes[node_id],
        )
        for node_id, depth in best_depth.items()
    }


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    unique_values: list[str] = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        unique_values.append(value)
    return tuple(unique_values)
