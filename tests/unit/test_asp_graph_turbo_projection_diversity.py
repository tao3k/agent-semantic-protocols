"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    TypedGraph,
    rank_frontier,
    render_compact,
)


def test_owner_query_projection_prefers_symbol_diverse_branches() -> None:
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
                    "id": "item:fields",
                    "kind": "item",
                    "role": "symbol",
                    "value": "fields",
                    "path": "tokio/src/io/driver/scheduled_io.rs",
                    "ownerPath": "tokio/src/io/driver/scheduled_io.rs",
                    "symbol": "fields",
                    "startLine": 480,
                    "endLine": 480,
                    "locator": "tokio/src/io/driver/scheduled_io.rs:480:480",
                    "matchText": "access the waker fields",
                },
                {
                    "id": "item:collection-a",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "weight": 1.5,
                    "path": "tokio/src/loom/std/mod.rs",
                    "ownerPath": "tokio/src/loom/std/mod.rs",
                    "symbol": "collection",
                    "startLine": 29,
                    "endLine": 29,
                    "locator": "tokio/src/loom/std/mod.rs:29:29",
                    "matchText": "collection implementation",
                },
                {
                    "id": "item:collection-b",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "weight": 1.4,
                    "path": "tokio/src/process/mod.rs",
                    "ownerPath": "tokio/src/process/mod.rs",
                    "symbol": "collection",
                    "startLine": 315,
                    "endLine": 315,
                    "locator": "tokio/src/process/mod.rs:315:315",
                    "matchText": "collection of child processes",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "stress-test/examples/simple_echo_tcp.rs",
                    "ownerPath": "stress-test/examples/simple_echo_tcp.rs",
                    "symbol": "vec",
                    "startLine": 131,
                    "endLine": 131,
                    "locator": "stress-test/examples/simple_echo_tcp.rs:131:131",
                    "matchText": "Vec buffer",
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "item:fields",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection-a",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection-b",
                    "relation": "matches",
                },
                {"source": "q:vec-fields", "target": "item:vec", "relation": "matches"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=5,
        kind_budgets={"query": 1, "item": 4},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert "symbol=fields" in frontier_actions
    assert frontier_actions.count("symbol=collection") == 1
    assert "symbol=vec" in frontier_actions


def test_owner_query_projection_dedupes_item_hot_mapped_selectors() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "src/read_dir.rs",
                    "ownerPath": "src/read_dir.rs",
                    "symbol": "vec",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/read_dir.rs:3:3",
                },
                {
                    "id": "hot:vec",
                    "kind": "hot",
                    "role": "range",
                    "value": "vec",
                    "path": "src/read_dir.rs",
                    "ownerPath": "src/read_dir.rs",
                    "symbol": "vec",
                    "startLine": 1,
                    "endLine": 15,
                    "locator": "src/read_dir.rs:1:15",
                },
            ],
            "edges": [
                {"source": "q:vec", "target": "item:vec", "relation": "matches"},
                {"source": "item:vec", "target": "hot:vec", "relation": "contains"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec"],
        limit=3,
        kind_budgets={"query": 1, "item": 1, "hot": 1},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert (
        "selectorPolicy=run-first reason=exact-selector-present before=search-reasoning"
        in compact
    )
    assert frontier_actions.count("selector=src/read_dir.rs:1:15") == 1
    assert frontier_actions.count(".selector(") == 1
    assert frontier_actions.index("S1.selector(") < frontier_actions.index(
        "R1.reasoning("
    )
