# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Public SciPy router for the bounded SearchLoop cost vector."""

from __future__ import annotations

from collections.abc import Mapping
from math import isfinite

from .graph_model import TypedGraph
from .path_scipy_searchloop_cost import add_cost, encoded_edges
from .path_scipy_searchloop_matrix import (
    ScipyPathProjection,
    best_edges_by_pair,
    scipy_shortest_path,
)
from .path_scipy_searchloop_model import (
    EncodedEdge,
    MixedRadix,
    SearchLoopCost,
    SearchLoopRoute,
)

__all__ = ["SearchLoopCost", "SearchLoopRoute", "searchloop_scipy_route"]


def _validate_route_request(
    graph: TypedGraph, source: str, sink: str, max_hops: int
) -> None:
    if isinstance(max_hops, bool) or not isinstance(max_hops, int) or max_hops < 0:
        raise ValueError("max_hops must be a non-negative integer")
    if source not in graph.nodes:
        raise KeyError(f"unknown source node: {source}")
    if sink not in graph.nodes:
        raise KeyError(f"unknown sink node: {sink}")


def _route_from_projection(
    projection: ScipyPathProjection,
    pair_edges: Mapping[tuple[str, str], EncodedEdge],
    radix: MixedRadix,
    *,
    max_hops: int,
) -> SearchLoopRoute | None:
    if not isfinite(projection.distance):
        return None
    path_indices = projection.path_indices()
    if not path_indices or len(path_indices) - 1 > max_hops:
        return None
    route_edges = tuple(
        pair_edges[(projection.node_ids[left], projection.node_ids[right])]
        for left, right in zip(path_indices, path_indices[1:])
    )
    cost = SearchLoopCost()
    for route_edge in route_edges:
        cost = add_cost(cost, route_edge.cost)
    encoded_cost = radix.encode(cost)
    if encoded_cost != int(projection.distance):
        raise RuntimeError("SciPy distance and audited SearchLoop cost diverged")
    return SearchLoopRoute(
        node_ids=tuple(projection.node_ids[index] for index in path_indices),
        relations=tuple(edge.edge.relation for edge in route_edges),
        cost=cost,
        encoded_cost=encoded_cost,
    )


def searchloop_scipy_route(
    graph: TypedGraph,
    source: str,
    sink: str,
    *,
    max_hops: int,
    allowed_relations: frozenset[str] | None = None,
) -> SearchLoopRoute | None:
    """Return the lexicographically minimal bounded SearchLoop route.

    The objective order is hops, interaction rounds, tokens, search-cache
    misses, then model-prefix-cache misses. Unknown cache state contributes no
    fabricated miss. Invalid cost data and unsafe numeric bounds fail closed;
    an unreachable sink returns ``None``. There is no algorithm fallback.
    """

    _validate_route_request(graph, source, sink, max_hops)
    if source == sink:
        return SearchLoopRoute(
            node_ids=(source,),
            relations=(),
            cost=SearchLoopCost(),
            encoded_cost=0,
        )
    if max_hops == 0:
        return None

    candidates, radix = encoded_edges(
        graph,
        allowed_relations=allowed_relations,
        max_hops=max_hops,
    )
    pair_edges = best_edges_by_pair(candidates)
    projection = scipy_shortest_path(graph, pair_edges, source=source, sink=sink)
    return _route_from_projection(
        projection,
        pair_edges,
        radix,
        max_hops=max_hops,
    )
