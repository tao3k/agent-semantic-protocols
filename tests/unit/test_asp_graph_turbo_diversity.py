"""Diversity ranking tests for graph-turbo."""

from __future__ import annotations

from asp_graph_turbo import Node, TypedGraph
from asp_graph_turbo.graph_model import Edge
from asp_graph_turbo.diversity import rank_nodes
from asp_graph_turbo.query_token_balance import query_tokens_for_seed_nodes


def test_rank_nodes_preserves_hot_companion_for_selected_item() -> None:
    graph = TypedGraph(
        nodes=[
            _node("query:weights", "query", "term", "weights"),
            _node(
                "item:weights",
                "item",
                "symbol",
                "weights",
                locator="src/backend.py:35:35",
                action="syntax",
            ),
            _node(
                "hot:weights",
                "hot",
                "range",
                "weights",
                locator="src/backend.py:27:47",
                action="code",
            ),
            _node(
                "item:profile",
                "item",
                "symbol",
                "profilecompatibility",
                locator="src/profile.py:8:8",
                action="syntax",
            ),
            _node(
                "item:transition",
                "item",
                "symbol",
                "transition",
                locator="src/transition.py:88:88",
                action="syntax",
            ),
        ]
    )

    ranked = rank_nodes(
        graph,
        {
            "query:weights": 10.0,
            "item:weights": 9.0,
            "hot:weights": 8.0,
            "item:profile": 8.1,
            "item:transition": 8.05,
        },
        {
            "query:weights": 0,
            "item:weights": 1,
            "hot:weights": 2,
            "item:profile": 1,
            "item:transition": 1,
        },
        4,
        {},
    )

    assert [node.id for node in ranked] == [
        "query:weights",
        "item:weights",
        "hot:weights",
        "item:profile",
    ]


def test_rank_nodes_balances_uncovered_query_tokens() -> None:
    graph = TypedGraph(
        nodes=[
            _node("query:alpha-beta", "query", "term", "alpha beta"),
            _node(
                "item:alpha-main",
                "item",
                "symbol",
                "alpha_main",
                locator="src/alpha.py:10:10",
            ),
            _node(
                "item:alpha-helper",
                "item",
                "symbol",
                "alpha_helper",
                locator="src/alpha.py:20:20",
            ),
            _node(
                "item:alpha-extra",
                "item",
                "symbol",
                "alpha_extra",
                locator="src/alpha.py:30:30",
            ),
            _node(
                "item:beta-rare",
                "item",
                "symbol",
                "beta_rare",
                locator="src/beta.py:40:40",
            ),
        ]
    )

    ranked = rank_nodes(
        graph,
        {
            "query:alpha-beta": 10.0,
            "item:alpha-main": 9.0,
            "item:alpha-helper": 8.1,
            "item:alpha-extra": 8.0,
            "item:beta-rare": 7.95,
        },
        {
            "query:alpha-beta": 0,
            "item:alpha-main": 1,
            "item:alpha-helper": 1,
            "item:alpha-extra": 1,
            "item:beta-rare": 1,
        },
        4,
        {},
        query_tokens=query_tokens_for_seed_nodes(graph, ("query:alpha-beta",)),
    )

    assert [node.id for node in ranked] == [
        "query:alpha-beta",
        "item:alpha-main",
        "item:beta-rare",
        "item:alpha-helper",
    ]


def test_rank_nodes_repairs_semantic_type_fact_coverage() -> None:
    graph = TypedGraph(
        nodes=[
            _node("query:effect-stream", "query", "term", "effect stream"),
            _node("field:embed-many", "field", "interface-field", "Effect concurrency"),
            _node("collection:array", "collection", "family", "array"),
            _node("item:stream-primary", "item", "symbol", "streamText"),
            _node("item:stream-extra", "item", "symbol", "streamMany"),
            _node("type:effect", "type", "field-type", "Effect.Effect<Array<string>>"),
        ],
        edges=[
            Edge("field:embed-many", "type:effect", "has_type"),
            Edge("field:embed-many", "collection:array", "collection_of"),
        ],
    )

    ranked = rank_nodes(
        graph,
        {
            "query:effect-stream": 10.0,
            "field:embed-many": 9.0,
            "collection:array": 8.8,
            "item:stream-primary": 8.6,
            "item:stream-extra": 8.5,
            "type:effect": 1.0,
        },
        {
            "query:effect-stream": 0,
            "field:embed-many": 1,
            "collection:array": 2,
            "item:stream-primary": 1,
            "item:stream-extra": 1,
            "type:effect": 2,
        },
        5,
        {},
        query_tokens=query_tokens_for_seed_nodes(graph, ("query:effect-stream",)),
    )

    ranked_ids = [node.id for node in ranked]
    assert "type:effect" in ranked_ids
    assert "field:embed-many" in ranked_ids
    assert "collection:array" in ranked_ids
    assert any("stream" in node.value.lower() for node in ranked)


def _node(
    node_id: str,
    kind: str,
    role: str,
    value: str,
    *,
    locator: str | None = None,
    action: str | None = None,
) -> Node:
    fields: dict[str, object] = {}
    if locator is not None:
        path, start, end = _parse_locator(locator)
        fields.update(
            {
                "path": path,
                "ownerPath": path,
                "symbol": value,
                "startLine": start,
                "endLine": end,
                "locator": locator,
            }
        )
    return Node(node_id, kind, role, value, action=action, fields=fields)


def _parse_locator(locator: str) -> tuple[str, int, int]:
    path, _, end_text = locator.rpartition(":")
    path, _, start_text = path.rpartition(":")
    return path, int(start_text), int(end_text)
