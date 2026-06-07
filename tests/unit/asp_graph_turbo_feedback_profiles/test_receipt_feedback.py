"""Receipt feedback ranking tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    item_node,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)


def test_receipt_graph_facts_adjust_ranking_and_packet_evidence() -> None:
    graph = TypedGraph.from_packet(_receipt_boost_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:parser"],
        limit=4,
        kind_budgets={"query": 1, "owner": 1, "item": 2},
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    ranked_items = [node_id for node_id in packet["rank"] if node_id.startswith("item:")]

    assert ranked_items[0] == "item:new"
    assert packet["algorithmMetrics"]["receiptBoostCount"] == 1
    assert packet["algorithmMetrics"]["receiptPenaltyCount"] == 2
    assert "seen-selector" in packet["avoid"]
    assert {
        "nodeId": "item:new",
        "effect": "boost",
        "scoreDelta": 0.45,
        "reason": "test-passed",
    } in packet["receiptAdjustments"]
    assert any(
        "receipt-boost:+0.45:test-passed" in explanation["reasons"]
        for explanation in packet["rankExplanations"]
    )
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_receipt_read_patterns_penalize_overlap_owner_and_raw_read_fallback() -> None:
    graph = TypedGraph.from_packet(_read_loop_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser"],
        limit=6,
        kind_budgets={"query": 1, "owner": 1, "item": 4},
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    adjustments = {
        (entry["nodeId"], entry["reason"]) for entry in packet["receiptAdjustments"]
    }

    assert ("item:seen", "seen-selector") in adjustments
    assert ("item:overlap", "same-range-overlap") in adjustments
    assert ("item:same-owner", "same-owner-scan") in adjustments
    assert ("item:same-owner", "raw-read-fallback") in adjustments
    assert packet["scores"]["item:clean"] > packet["scores"]["item:same-owner"]
    assert packet["algorithmMetrics"]["receiptPenaltyCount"] >= 6
    assert any(
        "receipt-penalty:-0.75:raw-read-fallback" in explanation["reasons"]
        for explanation in packet["rankExplanations"]
        if explanation["nodeId"] == "item:same-owner"
    )
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def _receipt_boost_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {
                "id": "owner:parser",
                "kind": "owner",
                "role": "path",
                "value": "src/parser.py",
                "path": "src/parser.py",
            },
            item_node("item:old", "parse_old", "src/parser.py:10:20"),
            item_node("item:new", "parse_new", "src/parser.py:30:40"),
            {
                "id": "receipt:old-read",
                "kind": "receipt",
                "role": "read",
                "value": "src/parser.py:10:20",
                "receiptKind": "read",
                "selector": "src/parser.py:10:20",
            },
            {
                "id": "receipt:new-test",
                "kind": "receipt",
                "role": "test",
                "value": "test passed after edit",
                "receiptKind": "test",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "owner:parser", "relation": "matches"},
            {"source": "q:parser", "target": "item:old", "relation": "matches"},
            {"source": "q:parser", "target": "item:new", "relation": "matches"},
            {"source": "owner:parser", "target": "item:old", "relation": "contains"},
            {"source": "owner:parser", "target": "item:new", "relation": "contains"},
            {"source": "receipt:old-read", "target": "item:old", "relation": "read"},
            {
                "source": "receipt:new-test",
                "target": "item:new",
                "relation": "test-passed",
            },
        ],
    }


def _read_loop_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {
                "id": "owner:parser",
                "kind": "owner",
                "role": "path",
                "value": "src/parser.py",
                "path": "src/parser.py",
            },
            item_node("item:seen", "parse_seen", "src/parser.py:10:20"),
            item_node("item:overlap", "parse_overlap", "src/parser.py:18:25"),
            item_node("item:same-owner", "parse_same_owner", "src/parser.py:80:90"),
            item_node("item:clean", "parse_clean", "src/clean.py:10:20"),
            {
                "id": "receipt:read-loop",
                "kind": "receipt",
                "role": "raw-read",
                "value": "src/parser.py",
                "receiptKind": "raw-read",
                "selector": "src/parser.py:10:20",
                "ownerPath": "src/parser.py",
                "avoidReasons": ["manual-window-scan", "repeat-owner", "raw-read"],
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "owner:parser", "relation": "matches"},
            {"source": "q:parser", "target": "item:seen", "relation": "matches"},
            {"source": "q:parser", "target": "item:overlap", "relation": "matches"},
            {"source": "q:parser", "target": "item:same-owner", "relation": "matches"},
            {"source": "q:parser", "target": "item:clean", "relation": "matches"},
            {"source": "owner:parser", "target": "item:seen", "relation": "contains"},
            {"source": "owner:parser", "target": "item:overlap", "relation": "contains"},
            {
                "source": "owner:parser",
                "target": "item:same-owner",
                "relation": "contains",
            },
        ],
    }
