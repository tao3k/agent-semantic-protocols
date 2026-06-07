"""Failure evidence profile tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    matrix,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)
from ._failure_packet import failure_evidence_graph_packet


def test_failure_evidence_profile_compiles_assert_to_fact_flow() -> None:
    graph = TypedGraph.from_packet(failure_evidence_graph_packet())
    result = rank_frontier(
        graph,
        profile="failure-evidence",
        seeds=["failure:cache"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "failure-evidence")

    assert packet["profile"] == "failure-evidence"
    assert profile_matrix["reachableEdgeCount"] >= 6
    assert "assert:replay" in packet["rank"]
    assert "field:entries" in packet["rank"]
    assert "collection:entries" in packet["rank"]
    assert "evidence:file-hash" in packet["rank"]
    assert any("checks" in path["relations"] for path in packet["typedPaths"])
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
