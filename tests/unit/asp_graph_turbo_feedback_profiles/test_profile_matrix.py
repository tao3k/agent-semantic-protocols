"""Profile matrix packet visibility tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    sample_packet,
    schema_validator_for,
)


def test_profile_matrix_bank_is_packet_visible_and_schema_valid() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:cli"],
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    matrices = {entry["profile"]: entry for entry in packet["profileMatrices"]}

    assert list(matrices) == packet["profiles"]
    assert matrices["owner-query"]["relationCount"] > 0
    assert matrices["owner-query"]["transitionCount"] > 0
    assert matrices["owner-query"]["supportedEdgeCount"] >= 4
    assert matrices["owner-query"]["reachableEdgeCount"] > 0
    assert matrices["owner-query"]["density"] > 0
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
