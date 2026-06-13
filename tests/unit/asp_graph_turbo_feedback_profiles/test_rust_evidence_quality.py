"""Rust evidence quality profile tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_REQUEST_SCHEMA,
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    matrix,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)
from ._rust_evidence_packet import rust_evidence_graph_turbo_request


def test_rust_evidence_quality_profile_ranks_nested_request_graph() -> None:
    request = rust_evidence_graph_turbo_request()
    assert list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(request)) == []

    graph = TypedGraph.from_packet(request)
    result = rank_frontier(
        graph,
        profile="rust-evidence-quality",
        seeds=["owner:src/model.rs"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "rust-evidence-quality")

    assert packet["profile"] == "rust-evidence-quality"
    assert "rust-evidence-quality" in packet["profiles"]
    assert profile_matrix["reachableEdgeCount"] >= 5
    assert "invariant:agent-r027" in packet["rank"]
    assert "gap:receipt" in packet["rank"]
    assert "receipt:cargo-check" in packet["rank"]
    reliability = packet["evidenceReliability"]
    assert reliability["reliable"] is False
    assert reliability["blockingCount"] == 1
    assert reliability["gates"] == ["collect-evidence"]
    assert {
        finding["kind"] for finding in reliability["findings"]
    } >= {"evidence-gap"}
    channels = {
        channel["relation"]: channel for channel in profile_matrix["relationChannels"]
    }
    assert channels["verified-by"]["reachableEdgeCount"] >= 1
    assert any("verified-by" in path["relations"] for path in packet["typedPaths"])
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
