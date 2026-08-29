"""Bounded mixed-radix encoding for SearchLoop edge costs."""

from __future__ import annotations

from typing import Mapping

from .graph_model import Edge, TypedGraph
from .path_scipy_searchloop_model import (
    MAX_EXACT_FLOAT_INTEGER,
    EncodedEdge,
    MixedRadix,
    SearchLoopCost,
)


def _integer_field(fields: Mapping[str, object], name: str, default: int) -> int:
    value = fields.get(name, default)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"edge field {name!r} must be a non-negative integer")
    return value


def _cache_miss(
    fields: Mapping[str, object], *, miss_name: str, hit_name: str
) -> int:
    if miss_name in fields:
        return _integer_field(fields, miss_name, 0)
    if hit_name not in fields:
        return 0
    hit = fields[hit_name]
    if not isinstance(hit, bool):
        raise ValueError(f"edge field {hit_name!r} must be a boolean")
    return int(not hit)


def searchloop_edge_cost(edge: Edge) -> SearchLoopCost:
    fields = edge.fields
    return SearchLoopCost(
        hops=1,
        interaction_rounds=_integer_field(fields, "interactionRounds", 1),
        tokens=_integer_field(fields, "tokenCost", 0),
        search_cache_misses=_cache_miss(
            fields,
            miss_name="searchCacheMisses",
            hit_name="searchCacheHit",
        ),
        model_cache_misses=_cache_miss(
            fields,
            miss_name="modelCacheMisses",
            hit_name="modelCacheHit",
        ),
    )


def radix_for(costs: tuple[SearchLoopCost, ...], max_hops: int) -> MixedRadix:
    max_rounds = max((cost.interaction_rounds for cost in costs), default=0)
    max_tokens = max((cost.tokens for cost in costs), default=0)
    max_search_misses = max((cost.search_cache_misses for cost in costs), default=0)
    max_model_misses = max((cost.model_cache_misses for cost in costs), default=0)
    return MixedRadix(
        rounds_base=max_hops * max_rounds + 1,
        tokens_base=max_hops * max_tokens + 1,
        search_cache_base=max_hops * max_search_misses + 1,
        model_cache_base=max_hops * max_model_misses + 1,
    )


def add_cost(left: SearchLoopCost, right: SearchLoopCost) -> SearchLoopCost:
    return SearchLoopCost(
        hops=left.hops + right.hops,
        interaction_rounds=left.interaction_rounds + right.interaction_rounds,
        tokens=left.tokens + right.tokens,
        search_cache_misses=left.search_cache_misses + right.search_cache_misses,
        model_cache_misses=left.model_cache_misses + right.model_cache_misses,
    )


def encoded_edges(
    graph: TypedGraph,
    *,
    allowed_relations: frozenset[str] | None,
    max_hops: int,
) -> tuple[tuple[EncodedEdge, ...], MixedRadix]:
    edges = tuple(
        edge
        for edge in graph.edges
        if allowed_relations is None or edge.relation in allowed_relations
    )
    costs = tuple(searchloop_edge_cost(edge) for edge in edges)
    radix = radix_for(costs, max_hops)
    encoded = tuple(
        EncodedEdge(edge=edge, cost=cost, encoded_cost=radix.encode(cost))
        for edge, cost in zip(edges, costs, strict=True)
    )
    maximum_path_cost = radix.encode(
        SearchLoopCost(
            hops=max_hops,
            interaction_rounds=max_hops
            * max((cost.interaction_rounds for cost in costs), default=0),
            tokens=max_hops * max((cost.tokens for cost in costs), default=0),
            search_cache_misses=max_hops
            * max((cost.search_cache_misses for cost in costs), default=0),
            model_cache_misses=max_hops
            * max((cost.model_cache_misses for cost in costs), default=0),
        )
    )
    if maximum_path_cost > MAX_EXACT_FLOAT_INTEGER:
        raise ValueError(
            "bounded SearchLoop cost cannot be represented exactly by SciPy float64"
        )
    return encoded, radix
