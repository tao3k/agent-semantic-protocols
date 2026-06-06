"""Policy tests for ASP graph turbo ranking weights and diversity."""

from __future__ import annotations

from asp_graph_turbo import TypedGraph, rank_frontier


def centrality_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {
                "id": "item:a",
                "kind": "item",
                "role": "symbol",
                "value": "semantic_string_type",
                "owner": "src/a.py",
            },
            {
                "id": "item:z",
                "kind": "item",
                "role": "symbol",
                "value": "semantic_string_type",
                "owner": "src/z.py",
            },
            {
                "id": "hot:z",
                "kind": "hot",
                "role": "call",
                "value": "command_intent",
                "owner": "src/z.py",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "item:a", "relation": "matches"},
            {"source": "q:parser", "target": "item:z", "relation": "matches"},
            {"source": "item:z", "target": "hot:z", "relation": "contains"},
        ],
    }


def query_deps_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {"id": "owner:a", "kind": "owner", "role": "path", "value": "src/a.py"},
            {"id": "owner:z", "kind": "owner", "role": "path", "value": "src/z.py"},
            {
                "id": "dep:z",
                "kind": "dependency",
                "role": "pkg",
                "value": "jsonschema",
            },
            {
                "id": "test:z",
                "kind": "test",
                "role": "path",
                "value": "tests/test_z.py",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "owner:a", "relation": "matches"},
            {"source": "q:parser", "target": "owner:z", "relation": "matches"},
            {"source": "owner:z", "target": "dep:z", "relation": "uses"},
            {"source": "owner:z", "target": "test:z", "relation": "covers"},
        ],
    }


def test_owner_query_diversity_penalizes_repeated_symbol_names() -> None:
    graph = TypedGraph.from_packet(centrality_packet())
    result = rank_frontier(graph, profile="owner-query", seeds=["q:parser"])

    ranked = [node.id for node in result.ranked_nodes]

    assert ranked.index("hot:z") < ranked.index("item:z")
    assert ranked.count("item:z") == 1


def test_kind_budget_limits_ranked_nodes_per_kind() -> None:
    graph = TypedGraph.from_packet(query_deps_packet())
    result = rank_frontier(
        graph,
        profile="query-deps",
        seeds=["q:parser"],
        kind_budgets={"owner": 1},
    )

    ranked_owner_ids = [node.id for node in result.ranked_nodes if node.kind == "owner"]

    assert ranked_owner_ids == ["owner:z"]
    assert result.kind_budgets == {"owner": 1}
