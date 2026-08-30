"""Read-loop second-pass replacement tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)


def test_read_loop_second_pass_replaces_same_owner_surplus_when_alternative_exists() -> (
    None
):
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {"id": "q:parse", "kind": "query", "role": "term", "value": "parse"},
                {
                    "id": "owner:parser",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/parser.py",
                    "path": "src/parser.py",
                },
                _item("item:head", "parse_head", "src/parser.py:10:20"),
                _item("item:body", "parse_body", "src/parser.py:40:50"),
                _item("item:tail", "parse_tail", "src/parser.py:80:90"),
                _item("item:other", "parse_other", "src/other.py:5:12"),
            ],
            "edges": [
                {"source": "q:parse", "target": "owner:parser", "relation": "matches"},
                {"source": "q:parse", "target": "item:head", "relation": "matches"},
                {"source": "q:parse", "target": "item:body", "relation": "matches"},
                {"source": "q:parse", "target": "item:tail", "relation": "matches"},
                {"source": "q:parse", "target": "item:other", "relation": "matches"},
                {
                    "source": "owner:parser",
                    "target": "item:head",
                    "relation": "contains",
                },
                {
                    "source": "owner:parser",
                    "target": "item:body",
                    "relation": "contains",
                },
                {
                    "source": "owner:parser",
                    "target": "item:tail",
                    "relation": "contains",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parse", "owner:parser"],
        limit=5,
        kind_budgets={"query": 1, "owner": 1, "item": 4},
    )
    packet = result_to_packet(result)

    assert "item:other" in packet["rank"]
    assert "item:tail" not in packet["rank"]
    assert packet["algorithmMetrics"]["readLoopSecondPassSuppressedCount"] == 1
    assert packet["algorithmMetrics"]["readLoopSameOwnerSuppressedCount"] == 1
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_read_loop_second_pass_merges_adjacent_ranges() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {"id": "q:parse", "kind": "query", "role": "term", "value": "parse"},
                {
                    "id": "owner:parser",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/parser.py",
                    "path": "src/parser.py",
                },
                _item("item:head", "parse_head", "src/parser.py:10:20"),
                _item("item:body", "parse_body", "src/parser.py:21:28"),
                _item("item:other", "parse_other", "src/parser.py:80:90"),
            ],
            "edges": [
                {"source": "q:parse", "target": "owner:parser", "relation": "matches"},
                {"source": "q:parse", "target": "item:head", "relation": "matches"},
                {"source": "q:parse", "target": "item:body", "relation": "matches"},
                {"source": "q:parse", "target": "item:other", "relation": "matches"},
                {
                    "source": "owner:parser",
                    "target": "item:head",
                    "relation": "contains",
                },
                {
                    "source": "owner:parser",
                    "target": "item:body",
                    "relation": "contains",
                },
                {
                    "source": "owner:parser",
                    "target": "item:other",
                    "relation": "contains",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parse", "owner:parser"],
        limit=4,
        kind_budgets={"query": 1, "owner": 1, "item": 3},
    )
    packet = result_to_packet(result)

    assert "item:other" in packet["rank"]
    assert not {"item:head", "item:body"} <= set(packet["rank"])
    assert packet["algorithmMetrics"]["readLoopSecondPassSuppressedCount"] == 1
    assert packet["algorithmMetrics"]["readLoopAdjacentRangeMergedCount"] == 1
    second_pass = next(
        step
        for step in packet["algorithmTrace"]
        if step["step"] == "read-loop-second-pass"
    )
    assert second_pass["fields"]["adjacentRangeMergedCount"] == 1
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def _item(node_id: str, symbol: str, locator: str) -> dict[str, object]:
    path, start, end = _parse_locator(locator)
    return {
        "id": node_id,
        "kind": "item",
        "role": "fn",
        "value": symbol,
        "path": path,
        "ownerPath": path,
        "symbol": symbol,
        "locator": locator,
        "startLine": start,
        "endLine": end,
    }


def _parse_locator(locator: str) -> tuple[str, int, int]:
    path, _, end_text = locator.rpartition(":")
    path, _, start_text = path.rpartition(":")
    return path, int(start_text), int(end_text)
