"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    TypedGraph,
    rank_frontier,
    sample_packet,
)


def test_owner_query_profile_masks_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph, profile="owner-query", seeds=["q:parser", "owner:cli"]
    )

    ranked = [node.id for node in result.ranked_nodes]

    assert "owner:cli" in ranked
    assert "item:collect" in ranked
    assert "hot:command" in ranked
    assert "test:cli" in ranked
    assert "dep:jsonschema" not in ranked
    assert ("dependency", "deps") not in [
        (entry.node.kind, entry.action) for entry in result.frontier
    ]


def test_owner_query_ranking_prefers_rare_query_token_match_text() -> None:
    nodes: list[dict[str, object]] = [
        {
            "id": "q:vec-collection",
            "kind": "query",
            "role": "term",
            "value": "Vec collection",
        },
        {
            "id": "item:collection",
            "kind": "item",
            "role": "symbol",
            "value": "collection",
            "path": "tokio/src/loom/std/mod.rs",
            "ownerPath": "tokio/src/loom/std/mod.rs",
            "symbol": "collection",
            "matchText": "collection helpers for loom std",
        },
    ]
    edges: list[dict[str, str]] = [
        {
            "source": "q:vec-collection",
            "target": "item:collection",
            "relation": "matches",
        }
    ]
    for index in range(8):
        node_id = f"item:vec-{index}"
        nodes.append(
            {
                "id": node_id,
                "kind": "item",
                "role": "symbol",
                "value": "vec",
                "path": f"tokio/src/fs/file_{index}.rs",
                "ownerPath": f"tokio/src/fs/file_{index}.rs",
                "symbol": "vec",
                "matchText": "let buffer: Vec<u8> = Vec::new();",
            }
        )
        edges.append(
            {
                "source": "q:vec-collection",
                "target": node_id,
                "relation": "matches",
            }
        )
    graph = TypedGraph.from_packet(
        {
            "nodes": nodes,
            "edges": edges,
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-collection"],
        limit=4,
        kind_budgets={"query": 1, "item": 3},
    )

    ranked_items = [node.id for node in result.ranked_nodes if node.kind == "item"]

    assert ranked_items[0] == "item:collection"
    assert ranked_items[1].startswith("item:vec-")
