# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Executable JSON witness for Python-Lean SearchLoop conformance."""

from __future__ import annotations

import json
import sys
from collections.abc import Mapping
from typing import Any

from .graph_model import Edge, Node, TypedGraph
from .path_scipy import SearchLoopCost, searchloop_scipy_route
from .path_scipy_searchloop_cost import radix_for, searchloop_edge_cost

SCHEMA_ID = "agent.semantic-protocols.searchloop-scipy-conformance"


def _graph(*edges: Edge) -> TypedGraph:
    node_ids = sorted({endpoint for edge in edges for endpoint in (edge.source, edge.target)})
    return TypedGraph(
        nodes=(Node(node_id, "item", "evidence", node_id) for node_id in node_ids),
        edges=edges,
    )


def _case(
    case_id: str,
    graph: TypedGraph,
    *,
    expected_winner: int,
    alternative_cost: SearchLoopCost,
    expected_alternative: int,
) -> Mapping[str, Any]:
    route = searchloop_scipy_route(graph, "source", "sink", max_hops=2)
    if route is None:
        raise RuntimeError(f"conformance case {case_id!r} is unreachable")
    radix = radix_for(
        tuple(searchloop_edge_cost(edge) for edge in graph.edges),
        max_hops=2,
    )
    actual_alternative = radix.encode(alternative_cost)
    passed = (
        route.encoded_cost == expected_winner
        and actual_alternative == expected_alternative
        and route.encoded_cost < actual_alternative
    )
    return {
        "caseId": case_id,
        "winner": route.encoded_cost,
        "alternative": actual_alternative,
        "expectedWinner": expected_winner,
        "expectedAlternative": expected_alternative,
        "selectedNodeIds": list(route.node_ids),
        "passed": passed,
    }


def searchloop_scipy_conformance_receipt() -> Mapping[str, Any]:
    direct_graph = _graph(
        Edge("source", "sink", "direct", fields={"tokenCost": 100}),
        Edge("source", "middle", "step", fields={"tokenCost": 0}),
        Edge("middle", "sink", "step", fields={"tokenCost": 0}),
    )
    round_graph = _graph(
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
    cache_graph = _graph(
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
    cases = [
        _case(
            "shorter-hop",
            direct_graph,
            expected_winner=904,
            alternative_cost=SearchLoopCost(hops=2, interaction_rounds=2),
            expected_alternative=1608,
        ),
        _case(
            "fewer-round",
            round_graph,
            expected_winner=1211,
            alternative_cost=SearchLoopCost(hops=2, interaction_rounds=3),
            expected_alternative=1313,
        ),
        _case(
            "cache-domain",
            cache_graph,
            expected_winner=19,
            alternative_cost=SearchLoopCost(hops=2, search_cache_misses=1),
            expected_alternative=21,
        ),
    ]
    return {
        "schemaId": SCHEMA_ID,
        "schemaVersion": "1",
        "objectiveOrder": [
            "hops",
            "interactionRounds",
            "tokens",
            "searchCacheMisses",
            "modelCacheMisses",
        ],
        "caseCount": len(cases),
        "passedCaseCount": sum(bool(case["passed"]) for case in cases),
        "passed": all(bool(case["passed"]) for case in cases),
        "cases": cases,
    }


def main() -> int:
    receipt = searchloop_scipy_conformance_receipt()
    sys.stdout.write(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")
    return 0 if receipt["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
