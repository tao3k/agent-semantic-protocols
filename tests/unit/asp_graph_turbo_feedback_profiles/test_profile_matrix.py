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
    assert (
        matrices["owner-query"]["relationMatrixCount"]
        == matrices["owner-query"]["relationCount"]
    )
    assert matrices["owner-query"]["transitionNonZeroCount"] > 0
    assert matrices["owner-query"]["transitionWeightMass"] > 0
    channels = {
        entry["relation"]: entry
        for entry in matrices["owner-query"]["relationChannels"]
    }
    assert len(channels) == matrices["owner-query"]["relationCount"]
    assert matrices["owner-query"]["zeroEdgeRelationCount"] == sum(
        1 for channel in channels.values() if channel["supportedEdgeCount"] == 0
    )
    assert (
        sum(channel["matrixNonZeroCount"] for channel in channels.values())
        == matrices["owner-query"]["supportedEdgeCount"]
    )
    assert channels["matches"]["supportedEdgeCount"] >= 1
    assert channels["matches"]["matrixNonZeroCount"] >= 1
    assert channels["contains"]["reachableEdgeCount"] >= 1
    assert channels["matches"]["weightMass"] > 0
    assert channels["matches"]["rankedContributionMass"] > 0
    assert channels["matches"]["frontierContributionMass"] > 0
    assert packet["algorithmMetrics"]["relationChannelCount"] == len(channels)
    assert packet["algorithmMetrics"]["pprIterations"] > 0
    typed_ppr = next(
        step for step in packet["algorithmTrace"] if step["step"] == "typed-ppr"
    )
    assert typed_ppr["fields"]["relationChannelCount"] == len(channels)
    assert typed_ppr["fields"]["iterations"] > 0
    assert typed_ppr["fields"]["massSum"] > 0
    item_explanation = next(
        explanation
        for explanation in packet["rankExplanations"]
        if explanation["nodeId"] == "item:collect"
    )
    assert "relation:matches:+1.50" in item_explanation["reasons"]
    assert any(reason.startswith("oriented:") for reason in item_explanation["reasons"])
    assert any(
        reason.startswith("relation:contains:")
        for reason in item_explanation["reasons"]
    )
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_oriented_edges_keep_original_direction_in_packet() -> None:
    graph_packet = sample_packet()
    graph_packet["edges"] = [
        edge
        for edge in graph_packet["edges"]
        if not (
            edge["source"] == "owner:cli"
            and edge["target"] == "item:collect"
            and edge["relation"] == "contains"
        )
    ]
    graph_packet["edges"].append(
        {"source": "item:collect", "target": "owner:cli", "relation": "contains"}
    )
    graph = TypedGraph.from_packet(graph_packet)
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:cli"],
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    contains_edges = [
        edge for edge in packet["edges"] if edge["relation"] == "contains"
    ]

    assert any(
        {
            "source": "owner:cli",
            "target": "item:collect",
            "relation": "contains",
            "originalSource": "item:collect",
            "originalTarget": "owner:cli",
            "reversed": True,
        }.items()
        <= edge.items()
        for edge in contains_edges
    )
    item_explanation = next(
        explanation
        for explanation in packet["rankExplanations"]
        if explanation["nodeId"] == "item:collect"
    )
    assert (
        "oriented:owner:cli>item:collect:contains:reversed"
        in item_explanation["reasons"]
    )
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
