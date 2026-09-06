# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Focused conformance tests for the SciPy SearchLoop router."""

from __future__ import annotations

import pytest

from asp_python_graphs import Edge, Node, TypedGraph
from asp_python_graphs.path_scipy import (
    SearchLoopCost,
    searchloop_scipy_route,
)
from asp_python_graphs.path_scipy_searchloop_cost import (
    radix_for,
    searchloop_edge_cost,
)
from asp_python_graphs.searchloop_scipy_conformance import (
    searchloop_scipy_conformance_receipt,
)


def _graph(*edges: Edge) -> TypedGraph:
    node_ids = sorted({endpoint for edge in edges for endpoint in (edge.source, edge.target)})
    return TypedGraph(
        nodes=(Node(node_id, "item", "evidence", node_id) for node_id in node_ids),
        edges=edges,
    )


def test_shorter_graph_path_dominates_lower_token_cost() -> None:
    graph = _graph(
        Edge("source", "sink", "direct", fields={"tokenCost": 100}),
        Edge("source", "middle", "step", fields={"tokenCost": 0}),
        Edge("middle", "sink", "step", fields={"tokenCost": 0}),
    )

    route = searchloop_scipy_route(graph, "source", "sink", max_hops=2)

    assert route is not None
    assert route.node_ids == ("source", "sink")
    assert route.cost == SearchLoopCost(hops=1, interaction_rounds=1, tokens=100)
    assert route.encoded_cost == 904
    radix = radix_for(
        tuple(searchloop_edge_cost(edge) for edge in graph.edges), max_hops=2
    )
    assert radix.encode(SearchLoopCost(hops=2, interaction_rounds=2)) == 1608


def test_equal_hops_minimize_rounds_before_tokens() -> None:
    graph = _graph(
        Edge(
            "source",
            "fast-round",
            "step",
            fields={"interactionRounds": 0, "tokenCost": 50},
        ),
        Edge("fast-round", "sink", "step", fields={"tokenCost": 50}),
        Edge(
            "source",
            "cheap-token",
            "step",
            fields={"interactionRounds": 2, "tokenCost": 0},
        ),
        Edge("cheap-token", "sink", "step", fields={"tokenCost": 0}),
    )

    route = searchloop_scipy_route(graph, "source", "sink", max_hops=2)

    assert route is not None
    assert route.node_ids == ("source", "fast-round", "sink")
    assert route.cost.interaction_rounds == 1
    assert route.cost.tokens == 100
    assert route.encoded_cost == 1211
    radix = radix_for(
        tuple(searchloop_edge_cost(edge) for edge in graph.edges), max_hops=2
    )
    assert radix.encode(SearchLoopCost(hops=2, interaction_rounds=3)) == 1313


def test_search_and_model_cache_misses_remain_distinct() -> None:
    graph = _graph(
        Edge(
            "source",
            "search-hit",
            "step",
            fields={"interactionRounds": 0, "searchCacheHit": True},
        ),
        Edge(
            "search-hit",
            "sink",
            "step",
            fields={
                "interactionRounds": 0,
                "searchCacheHit": True,
                "modelCacheHit": False,
            },
        ),
        Edge(
            "source",
            "model-hit",
            "step",
            fields={"interactionRounds": 0, "modelCacheHit": True},
        ),
        Edge(
            "model-hit",
            "sink",
            "step",
            fields={
                "interactionRounds": 0,
                "searchCacheHit": False,
                "modelCacheHit": True,
            },
        ),
    )

    route = searchloop_scipy_route(graph, "source", "sink", max_hops=2)

    assert route is not None
    assert route.node_ids == ("source", "search-hit", "sink")
    assert route.cost.search_cache_misses == 0
    assert route.cost.model_cache_misses == 1
    assert route.encoded_cost == 19
    radix = radix_for(
        tuple(searchloop_edge_cost(edge) for edge in graph.edges), max_hops=2
    )
    assert radix.encode(SearchLoopCost(hops=2, search_cache_misses=1)) == 21


def test_duplicate_pair_keeps_lexicographically_best_relation() -> None:
    graph = _graph(
        Edge("source", "sink", "z-expensive", fields={"tokenCost": 10}),
        Edge("source", "sink", "a-cheap", fields={"tokenCost": 1}),
    )

    route = searchloop_scipy_route(graph, "source", "sink", max_hops=1)

    assert route is not None
    assert route.relations == ("a-cheap",)
    assert route.cost.tokens == 1


def test_unsafe_float64_scalarization_fails_closed() -> None:
    graph = _graph(
        Edge("source", "sink", "step", fields={"tokenCost": 1 << 53}),
    )

    with pytest.raises(ValueError, match="cannot be represented exactly"):
        searchloop_scipy_route(graph, "source", "sink", max_hops=1)


def test_invalid_negative_metric_does_not_fall_back() -> None:
    graph = _graph(
        Edge("source", "sink", "step", fields={"interactionRounds": -1}),
    )

    with pytest.raises(ValueError, match="non-negative integer"):
        searchloop_scipy_route(graph, "source", "sink", max_hops=1)


def test_unreachable_route_returns_none() -> None:
    graph = TypedGraph(
        nodes=(
            Node("source", "item", "evidence", "source"),
            Node("sink", "item", "evidence", "sink"),
        )
    )

    assert searchloop_scipy_route(graph, "source", "sink", max_hops=2) is None


def test_python_lean_conformance_receipt_is_green() -> None:
    receipt = searchloop_scipy_conformance_receipt()

    assert receipt["passed"] is True
    assert receipt["caseCount"] == 3
    assert receipt["passedCaseCount"] == 3
    assert [
        (case["winner"], case["alternative"])
        for case in receipt["cases"]
    ] == [(904, 1608), (1211, 1313), (19, 21)]
