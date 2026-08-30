"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    TypedGraph,
    rank_frontier,
    render_compact,
)


def test_owner_query_projection_stops_after_provider_field_branches() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec collection fields",
                },
                {
                    "id": "field:snapshot-scalars",
                    "kind": "field",
                    "role": "struct-field",
                    "value": "scalars: Vec<Scalar>",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "scalars",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/lib.rs:3:3",
                    "matchText": "Snapshot::scalars: Vec<Scalar>",
                    "fields": {
                        "containerName": "Snapshot",
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                    },
                },
                {
                    "id": "type:snapshot-scalars-vec",
                    "kind": "type",
                    "role": "field-type",
                    "value": "Vec<Scalar>",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "Vec",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/lib.rs:3:3",
                    "fields": {
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                    },
                },
                {
                    "id": "collection:vec",
                    "kind": "collection",
                    "role": "family",
                    "value": "Vec",
                    "symbol": "Vec",
                },
                {
                    "id": "item:collection",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "collection",
                    "startLine": 4,
                    "endLine": 4,
                    "locator": "src/lib.rs:4:4",
                    "matchText": "lookup: HashMap<String, Scalar>",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "vec",
                    "startLine": 5,
                    "endLine": 5,
                    "locator": "src/lib.rs:5:5",
                    "matchText": "cursor: Cursor<Vec<u8>>",
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "field:snapshot-scalars",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "type:snapshot-scalars-vec",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "collection:vec",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection",
                    "relation": "matches",
                },
                {"source": "q:vec-fields", "target": "item:vec", "relation": "matches"},
                {
                    "source": "field:snapshot-scalars",
                    "target": "type:snapshot-scalars-vec",
                    "relation": "has_type",
                },
                {
                    "source": "field:snapshot-scalars",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
                {
                    "source": "type:snapshot-scalars-vec",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=8,
        kind_budgets={"query": 1, "field": 2, "type": 2, "collection": 2, "item": 4},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert "symbol=scalars" in frontier_actions
    assert "symbol=collection" not in frontier_actions
    assert "symbol=vec" not in frontier_actions
    assert frontier_actions.count(".selector(") == 1
    ranked_ids = [node.id for node in result.ranked_nodes]
    assert ranked_ids.index("field:snapshot-scalars") < ranked_ids.index(
        "collection:vec"
    )


def test_owner_query_projection_keeps_unscored_semantic_fact_nodes() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec collection fields",
                },
                {
                    "id": "item:buffer",
                    "kind": "item",
                    "role": "symbol",
                    "value": "buffer",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "buffer",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/lib.rs:3:3",
                    "matchText": "let buffer = Vec::new();",
                    "fields": {
                        "collectionKind": "Vec",
                    },
                },
                {
                    "id": "collection:vec",
                    "kind": "collection",
                    "role": "family",
                    "value": "Vec",
                    "symbol": "Vec",
                    "fields": {"collectionKind": "Vec"},
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "item:buffer",
                    "relation": "matches",
                },
                {
                    "source": "item:buffer",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
            ],
        }
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=3,
        kind_budgets={"query": 1, "item": 1, "collection": 1},
    )
    compact = render_compact(result)

    assert any(node.id == "collection:vec" for node in result.ranked_nodes)
    assert any(
        entry.node.id == "collection:vec" and entry.score == 0.0
        for entry in result.frontier
    )
    assert "collection:vec" not in result.scores
    assert "frontierActions=" in compact
