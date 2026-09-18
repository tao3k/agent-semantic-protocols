# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Field and collection impact profile tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    matrix,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)
from ._impact_packet import impact_graph_packet


def test_field_impact_profile_compiles_owner_field_type_hot_test_flow() -> None:
    graph = TypedGraph.from_packet(impact_graph_packet())
    result = rank_frontier(
        graph,
        profile="field-impact",
        seeds=["q:impact", "owner:cache"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "field-impact")

    assert packet["profile"] == "field-impact"
    assert profile_matrix["reachableEdgeCount"] >= 5
    assert "field:entries" in packet["rank"]
    assert "hot:write" in packet["rank"]
    channels = {
        channel["relation"]: channel for channel in profile_matrix["relationChannels"]
    }
    assert channels["has_type"]["reachableEdgeCount"] >= 1
    path_relations = {
        relation for path in packet["typedPaths"] for relation in path["relations"]
    }
    assert {"has_type", "relates", "covered_by"} & path_relations
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_collection_impact_profile_compiles_collection_mutation_flow() -> None:
    graph = TypedGraph.from_packet(impact_graph_packet())
    result = rank_frontier(
        graph,
        profile="collection-impact",
        seeds=["q:collection"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "collection-impact")

    assert packet["profile"] == "collection-impact"
    assert profile_matrix["supportedEdgeCount"] >= 4
    assert "collection:entries" in packet["rank"]
    assert "hot:mutate" in packet["rank"]
    channels = {
        channel["relation"]: channel for channel in profile_matrix["relationChannels"]
    }
    assert channels["collection_of"]["reachableEdgeCount"] >= 1
    assert channels["collection_of"]["reachableWeightMass"] > 0
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
