"""Package/build/test profile tests."""

from __future__ import annotations

from ._common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    matrix,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)
from ._package_packet import (
    build_test_package_graph_packet,
    provider_bridge_package_graph_packet,
)


def test_test_selection_profile_compiles_changed_fact_to_covering_test() -> None:
    graph = TypedGraph.from_packet(build_test_package_graph_packet())
    result = rank_frontier(
        graph,
        profile="test-selection",
        seeds=["q:cache", "field:entries"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "test-selection")

    assert packet["profile"] == "test-selection"
    assert profile_matrix["reachableEdgeCount"] >= 4
    assert "test:cache-unit" in packet["rank"]
    assert "build:cache-tests" in packet["rank"]
    assert any(entry["action"] == "tests" for entry in packet["frontier"])
    selected_relations = {edge["relation"] for edge in packet["edges"]}
    assert {"builds", "tests"}.issubset(selected_relations)
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_affected_profile_compiles_package_build_test_dependency_frontier() -> None:
    graph = TypedGraph.from_packet(build_test_package_graph_packet())
    result = rank_frontier(
        graph,
        profile="affected",
        seeds=["owner:cache"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    profile_matrix = matrix(packet, "affected")

    assert packet["profile"] == "affected"
    assert profile_matrix["reachableEdgeCount"] >= 4
    assert "package:cache" in packet["rank"]
    assert "build:cache-tests" in packet["rank"]
    assert "test:cache-unit" in packet["rank"]
    assert "dependency:serde" in packet["rank"]
    assert any(entry["action"] == "build" for entry in packet["frontier"])
    assert any(entry["action"] == "package" for entry in packet["frontier"])
    assert any("depends_on" in path["relations"] for path in packet["typedPaths"])
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_test_selection_profile_uses_provider_field_package_bridge() -> None:
    graph = TypedGraph.from_packet(provider_bridge_package_graph_packet())
    result = rank_frontier(
        graph,
        profile="test-selection",
        seeds=["field:entries"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)

    assert "package:cache" in packet["rank"]
    assert "build:cache-tests" in packet["rank"]
    assert "test:cache-unit" in packet["rank"]
    assert any(entry["action"] == "tests" for entry in packet["frontier"])
    assert any("belongs_to" in path["relations"] for path in packet["typedPaths"])
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_affected_profile_uses_provider_field_package_bridge_for_dependencies() -> None:
    graph = TypedGraph.from_packet(provider_bridge_package_graph_packet())
    result = rank_frontier(
        graph,
        profile="affected",
        seeds=["field:entries"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)

    assert "package:cache" in packet["rank"]
    assert "build:cache-tests" in packet["rank"]
    assert "test:cache-unit" in packet["rank"]
    assert "dependency:serde" in packet["rank"]
    assert any(entry["action"] == "deps" for entry in packet["frontier"])
    assert any("depends_on" in path["relations"] for path in packet["typedPaths"])
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
