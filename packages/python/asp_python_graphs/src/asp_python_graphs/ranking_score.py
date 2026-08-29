"""Score graph-turbo frontier nodes."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .cache import backend_fingerprint, cached_sparse_backend
from .model import GraphCache, GraphProfile, OrientedEdge, ReceiptAdjustment, TypedGraph
from .pagerank import GraphTurboPprResult
from .query_adjustments import (
    normalize_query_adjustment_policy,
    query_adjustments_by_node,
)
from .query_weights import (
    query_node_match_bonus,
    query_seed_personalization_weights,
    query_token_weights,
)
from .receipt import receipt_score_adjustments
from .runtime_cache import cached_hop_lengths, cached_pagerank, cached_reachable_edges


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
    cache_enabled: bool,
    query_clauses: Iterable[str] = (),
    query_adjustment_policy: Mapping[str, object] | None = None,
) -> tuple[
    dict[str, float],
    dict[str, int],
    dict[tuple[str, str, str], OrientedEdge],
    GraphCache,
    tuple[ReceiptAdjustment, ...],
    GraphTurboPprResult,
    Mapping[str, Mapping[str, float]],
    Mapping[str, str],
]:
    query_adjustment_policy = normalize_query_adjustment_policy(query_adjustment_policy)
    seed_id_tuple = tuple(seed_ids)
    backend, graph_cache = cached_sparse_backend(
        graph, profile, backend_fingerprint(graph, profile), enabled=cache_enabled
    )
    best_depth, depth_cache_status = cached_hop_lengths(
        graph_cache.key,
        backend,
        seed_id_tuple,
        profile.max_depth,
        enabled=cache_enabled,
    )
    seed_weights = (
        query_seed_personalization_weights(
            graph,
            profile_name=profile.name,
            seed_ids=seed_id_tuple,
        )
        if query_adjustment_policy["seedPrior"]
        else {}
    )
    pagerank, ppr_cache_status = cached_pagerank(
        graph_cache.key,
        backend,
        seed_id_tuple,
        seed_weights,
        enabled=cache_enabled,
    )
    query_adjustments = query_adjustments_by_node(
        graph,
        profile_name=profile.name,
        seed_ids=seed_id_tuple,
        query_clauses=query_clauses,
        policy=query_adjustment_policy,
    )
    scores = score_nodes(
        graph,
        profile,
        seed_id_tuple,
        best_depth,
        pagerank.scores,
        query_clauses=query_clauses,
        query_adjustment_policy=query_adjustment_policy,
        query_adjustments=query_adjustments,
    )
    receipt_adjustments, receipt_facts = receipt_score_adjustments(graph)
    for node_id, score_delta in receipt_adjustments.items():
        if node_id in scores:
            scores[node_id] += score_delta
    selected_edges, reachable_edges_cache_status = cached_reachable_edges(
        graph_cache.key,
        backend,
        seed_id_tuple,
        profile.max_depth,
        best_depth,
        enabled=cache_enabled,
    )
    return (
        scores,
        best_depth,
        selected_edges,
        graph_cache,
        receipt_facts,
        pagerank,
        query_adjustments,
        {
            "depthCacheStatus": depth_cache_status,
            "pprCacheStatus": ppr_cache_status,
            "reachableEdgesCacheStatus": reachable_edges_cache_status,
        },
    )


def score_nodes(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: Iterable[str],
    best_depth: Mapping[str, int],
    pagerank: Mapping[str, float],
    *,
    query_clauses: Iterable[str] = (),
    query_adjustment_policy: Mapping[str, object] | None = None,
    query_adjustments: Mapping[str, Mapping[str, float]] | None = None,
) -> dict[str, float]:
    query_adjustment_policy = normalize_query_adjustment_policy(query_adjustment_policy)
    query_adjustments = query_adjustments or query_adjustments_by_node(
        graph,
        profile_name=profile.name,
        seed_ids=seed_ids,
        query_clauses=query_clauses,
        policy=query_adjustment_policy,
    )
    max_pagerank = max(
        (pagerank.get(node_id, 0.0) for node_id in best_depth),
        default=0.0,
    )
    token_weights = query_token_weights(
        graph,
        profile_name=profile.name,
        seed_ids=seed_ids,
        query_clauses=query_clauses,
    )
    return {
        node_id: _node_score(
            graph,
            profile,
            node_id,
            depth,
            pagerank,
            max_pagerank,
            token_weights,
            query_adjustments,
        )
        for node_id, depth in best_depth.items()
    }


def _node_score(
    graph: TypedGraph,
    profile: GraphProfile,
    node_id: str,
    depth: int,
    pagerank: Mapping[str, float],
    max_pagerank: float,
    token_weights: Mapping[str, float],
    query_adjustments: Mapping[str, Mapping[str, float]],
) -> float:
    node = graph.nodes[node_id]
    node_adjustments = query_adjustments.get(node_id, {})
    return (
        node.weight
        + (float(profile.max_depth - depth + 1) * 0.2)
        + (pagerank.get(node_id, 0.0) / max_pagerank if max_pagerank > 0.0 else 0.0)
        + profile.kind_bonus.get(node.kind, 0.0)
        + query_node_match_bonus(
            profile_name=profile.name,
            token_weights=token_weights,
            node=node,
        )
        + node_adjustments.get("packageCohesion", 0.0)
        + node_adjustments.get("queryClauseCoverage", 0.0)
        + node_adjustments.get("localEvidence", 0.0)
        + node_adjustments.get("topologyMembership", 0.0)
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
