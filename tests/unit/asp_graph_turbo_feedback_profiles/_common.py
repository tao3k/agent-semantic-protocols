"""Shared helpers for graph-turbo feedback profile tests."""

from __future__ import annotations

from unit._asp_graph_turbo_common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    sample_packet,
    schema_validator_for,
)

__all__ = [
    "TypedGraph",
    "_GRAPH_TURBO_SCHEMA",
    "item_node",
    "matrix",
    "rank_frontier",
    "result_to_packet",
    "sample_packet",
    "schema_validator_for",
]


def matrix(packet: dict[str, object], profile: str) -> dict[str, object]:
    matrices = packet["profileMatrices"]
    assert isinstance(matrices, list)
    for entry in matrices:
        assert isinstance(entry, dict)
        if entry["profile"] == profile:
            return entry
    raise AssertionError(profile)


def item_node(node_id: str, symbol: str, locator: str) -> dict[str, object]:
    path, start, end = _parse_test_locator(locator)
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


def _parse_test_locator(locator: str) -> tuple[str, int, int]:
    path, _, end_text = locator.rpartition(":")
    path, _, start_text = path.rpartition(":")
    return path, int(start_text), int(end_text)
