"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    render_compact,
    result_to_packet,
    sample_packet,
    schema_validator_for,
)


def test_read_loop_guard_projects_repeated_code_followups() -> None:
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
                {
                    "id": "item:head",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_head",
                    "path": "src/parser.py",
                    "ownerPath": "src/parser.py",
                    "symbol": "parse_head",
                    "locator": "src/parser.py:10:20",
                },
                {
                    "id": "item:body",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_body",
                    "path": "src/parser.py",
                    "ownerPath": "src/parser.py",
                    "symbol": "parse_body",
                    "locator": "src/parser.py:21:28",
                },
                {
                    "id": "item:body-dup",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_body",
                    "path": "src/parser.py",
                    "ownerPath": "src/parser.py",
                    "symbol": "parse_body",
                    "locator": "src/parser.py:21:28",
                },
                {
                    "id": "item:tail",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_tail",
                    "path": "src/parser.py",
                    "ownerPath": "src/parser.py",
                    "symbol": "parse_tail",
                    "locator": "src/parser.py:29:34",
                },
            ],
            "edges": [
                {"source": "q:parse", "target": "owner:parser", "relation": "matches"},
                {"source": "q:parse", "target": "item:head", "relation": "matches"},
                {"source": "q:parse", "target": "item:body", "relation": "matches"},
                {"source": "q:parse", "target": "item:body-dup", "relation": "matches"},
                {"source": "q:parse", "target": "item:tail", "relation": "matches"},
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
                    "target": "item:body-dup",
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
        limit=6,
        kind_budgets={"query": 1, "owner": 1, "item": 4},
    )
    compact = render_compact(result)
    packet = result_to_packet(result)
    metrics = packet["algorithmMetrics"]

    assert "\navoid=raw-read,repeat-owner,broad-lexical,manual-window-scan\n" in compact
    assert "readLoop=code:3|duplicate:0|adjacent:2|sameOwner:2" in compact
    assert "readLoopSecondPass=duplicate:1|adjacentMerged:0|sameOwner:0" in compact
    assert metrics["readLoopDirectCodeActionCount"] == 3
    assert metrics["readLoopDuplicateSelectorCount"] == 0
    assert metrics["readLoopAdjacentRangeWindowCount"] == 2
    assert metrics["readLoopSameOwnerScanCount"] == 2
    assert metrics["readLoopSecondPassSuppressedCount"] == 1
    assert metrics["readLoopDuplicateSelectorSuppressedCount"] == 1
    assert metrics["readLoopAdjacentRangeMergedCount"] == 0
    assert metrics["readLoopSameOwnerSuppressedCount"] == 0
    assert "item:body-dup" not in packet["rank"]
    assert any(step.step == "read-loop-guard" for step in result.algorithm_trace)
    assert any(step.step == "read-loop-second-pass" for step in result.algorithm_trace)
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_read_memory_suppresses_seen_selector_from_frontier() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:cli"],
        seen_selectors=["src/cli.py:10:20"],
    )
    compact = render_compact(result)
    packet = result_to_packet(result)

    assert "item:collect" not in packet["rank"]
    assert (
        "readMemory=seen=src/cli.py:10:20 "
        "suppressed=src/cli.py:10:20,src/cli.py:24:28"
    ) in compact
    assert packet["readMemory"] == {
        "seenSelectors": ["src/cli.py:10:20"],
        "suppressedSelectors": ["src/cli.py:10:20", "src/cli.py:24:28"],
    }
    assert packet["algorithmMetrics"]["readMemorySuppressedCount"] == 2
    assert "seen-selector" in packet["avoid"]
    assert "\navoid=" in compact and "seen-selector" in compact
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_read_memory_suppresses_adjacent_seen_range_from_frontier() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {"id": "q:parse", "kind": "query", "role": "term", "value": "parse"},
                {
                    "id": "owner:cli",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/cli.py",
                    "path": "src/cli.py",
                },
                {
                    "id": "item:seen",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_seen",
                    "path": "src/cli.py",
                    "ownerPath": "src/cli.py",
                    "symbol": "parse_seen",
                    "locator": "src/cli.py:10:20",
                },
                {
                    "id": "item:adjacent",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_adjacent",
                    "path": "src/cli.py",
                    "ownerPath": "src/cli.py",
                    "symbol": "parse_adjacent",
                    "locator": "src/cli.py:18:26",
                },
                {
                    "id": "item:far",
                    "kind": "item",
                    "role": "fn",
                    "value": "parse_far",
                    "path": "src/cli.py",
                    "ownerPath": "src/cli.py",
                    "symbol": "parse_far",
                    "locator": "src/cli.py:60:70",
                },
            ],
            "edges": [
                {"source": "q:parse", "target": "owner:cli", "relation": "matches"},
                {"source": "q:parse", "target": "item:seen", "relation": "matches"},
                {"source": "q:parse", "target": "item:adjacent", "relation": "matches"},
                {"source": "q:parse", "target": "item:far", "relation": "matches"},
                {"source": "owner:cli", "target": "item:seen", "relation": "contains"},
                {"source": "owner:cli", "target": "item:adjacent", "relation": "contains"},
                {"source": "owner:cli", "target": "item:far", "relation": "contains"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parse", "owner:cli"],
        seen_selectors=["src/cli.py:10:20"],
    )
    compact = render_compact(result)
    packet = result_to_packet(result)

    assert "item:seen" not in packet["rank"]
    assert "item:adjacent" not in packet["rank"]
    assert "item:far" in packet["rank"]
    assert (
        "readMemory=seen=src/cli.py:10:20 "
        "suppressed=src/cli.py:10:20,src/cli.py:18:26"
    ) in compact
    assert packet["readMemory"] == {
        "seenSelectors": ["src/cli.py:10:20"],
        "suppressedSelectors": ["src/cli.py:10:20", "src/cli.py:18:26"],
    }
    assert packet["algorithmMetrics"]["readMemorySuppressedCount"] == 2
    assert "seen-selector" in packet["avoid"]
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
