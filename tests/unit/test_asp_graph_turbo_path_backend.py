"""Typed path backend selection tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    sample_packet,
    schema_validator_for,
)
from asp_graph_turbo import SourceSinkFrontier
from asp_graph_turbo.path_scipy import graph_turbo_scipy_yen_path_candidates
from asp_graph_turbo.profiles import resolve_profile


def test_small_graph_keeps_python_bfs_path_backend() -> None:
    result = rank_frontier(
        TypedGraph.from_packet(sample_packet()),
        profile="owner-query",
        seeds=["q:parser", "owner:cli"],
        cache_enabled=False,
    )

    typed_paths = [
        step for step in result.algorithm_trace if step.step == "typed-paths"
    ]

    assert typed_paths[0].engine == "python-bfs-small"
    assert result.algorithm_metrics.path_backend == "python-bfs-small"
    assert result.algorithm_metrics.path_fallback_count == 0
    assert result.algorithm_metrics.path_pair_count == 6
    assert result.algorithm_metrics.path_candidate_count == 8


def test_large_graph_uses_scipy_yen_path_backend() -> None:
    graph = TypedGraph.from_packet(_large_path_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:large"],
        limit=4,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    typed_paths = [
        step for step in packet["algorithmTrace"] if step["step"] == "typed-paths"
    ]

    assert typed_paths[0]["engine"] == "scipy-yen"
    assert typed_paths[0]["fields"]["fallbackCount"] == 0
    assert typed_paths[0]["fields"]["pairCount"] == 1
    assert typed_paths[0]["fields"]["candidateCount"] == 1
    assert packet["algorithmMetrics"]["pathBackend"] == "scipy-yen"
    assert packet["algorithmMetrics"]["pathFallbackCount"] == 0
    assert packet["algorithmMetrics"]["pathPairCount"] == 1
    assert packet["algorithmMetrics"]["pathCandidateCount"] == 1
    assert packet["typedPaths"][0]["relations"] == ["matches"]
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_large_graph_without_paths_uses_python_bfs_fallback_trace() -> None:
    graph = TypedGraph.from_packet(_large_unreachable_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:large"],
        limit=4,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    typed_paths = [
        step for step in packet["algorithmTrace"] if step["step"] == "typed-paths"
    ]

    assert typed_paths[0]["engine"] == "python-bfs-fallback"
    assert typed_paths[0]["fields"]["fallbackCount"] == 2
    assert typed_paths[0]["fields"]["pairCount"] == 0
    assert typed_paths[0]["fields"]["candidateCount"] == 0
    assert packet["algorithmMetrics"]["pathBackend"] == "python-bfs-fallback"
    assert packet["algorithmMetrics"]["pathFallbackCount"] == 2
    assert packet["algorithmMetrics"]["pathPairCount"] == 0
    assert packet["algorithmMetrics"]["pathCandidateCount"] == 0
    assert packet["typedPaths"] == []
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []


def test_scipy_yen_backend_projects_k_shortest_paths() -> None:
    graph = TypedGraph.from_packet(_large_k_shortest_packet())
    candidates = graph_turbo_scipy_yen_path_candidates(
        graph,
        resolve_profile("field-impact"),
        SourceSinkFrontier(("q:large",), ("hot:target",)),
        max_hops=3,
        path_budget=3,
    )
    path_node_ids = {candidate[2] for candidate in candidates}
    relation_paths = {candidate[3] for candidate in candidates}

    assert ("q:large", "field:direct", "hot:target") in path_node_ids
    assert ("q:large", "field:bridge", "hot:target") in path_node_ids
    assert ("matches", "contains") in relation_paths


def _large_path_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:large", "kind": "query", "role": "term", "value": "large"},
            {
                "id": "item:target",
                "kind": "item",
                "role": "fn",
                "value": "large_target",
                "path": "src/large.py",
                "ownerPath": "src/large.py",
                "symbol": "large_target",
                "locator": "src/large.py:10:20",
            },
            *[
                {
                    "id": f"item:filler-{index}",
                    "kind": "item",
                    "role": "fn",
                    "value": f"filler_{index}",
                }
                for index in range(50)
            ],
        ],
        "edges": [
            {"source": "q:large", "target": "item:target", "relation": "matches"},
        ],
    }


def _large_unreachable_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:large", "kind": "query", "role": "term", "value": "large"},
            {
                "id": "item:target",
                "kind": "item",
                "role": "fn",
                "value": "large_target",
                "path": "src/large.py",
                "ownerPath": "src/large.py",
                "symbol": "large_target",
                "locator": "src/large.py:10:20",
            },
            *[
                {
                    "id": f"item:filler-{index}",
                    "kind": "item",
                    "role": "fn",
                    "value": f"filler_{index}",
                }
                for index in range(50)
            ],
        ],
        "edges": [],
    }


def _large_k_shortest_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:large", "kind": "query", "role": "term", "value": "large"},
            {
                "id": "field:direct",
                "kind": "field",
                "role": "field",
                "value": "large_direct",
                "path": "src/large.py",
                "ownerPath": "src/large.py",
                "symbol": "large_direct",
                "locator": "src/large.py:10:20",
            },
            {
                "id": "field:bridge",
                "kind": "field",
                "role": "field",
                "value": "large_bridge",
                "path": "src/large.py",
                "ownerPath": "src/large.py",
                "symbol": "large_bridge",
                "locator": "src/large.py:24:32",
            },
            {
                "id": "hot:target",
                "kind": "hot",
                "role": "fn",
                "value": "large_target",
                "path": "src/large.py",
                "ownerPath": "src/large.py",
                "symbol": "large_target",
                "locator": "src/large.py:40:52",
            },
            *[
                {
                    "id": f"item:filler-{index}",
                    "kind": "item",
                    "role": "fn",
                    "value": f"filler_{index}",
                }
                for index in range(50)
            ],
        ],
        "edges": [
            {"source": "q:large", "target": "field:direct", "relation": "matches"},
            {"source": "q:large", "target": "field:bridge", "relation": "matches"},
            {"source": "field:direct", "target": "hot:target", "relation": "contains"},
            {
                "source": "field:bridge",
                "target": "hot:target",
                "relation": "contains",
            },
        ],
    }
