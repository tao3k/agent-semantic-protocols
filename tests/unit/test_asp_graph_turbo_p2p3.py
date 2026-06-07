"""P2/P3 tests for ASP graph turbo path, flow, cache, and trace evidence."""

from __future__ import annotations

from asp_graph_turbo import TypedGraph, rank_frontier
from asp_graph_turbo.cache import _BACKEND_CACHE


def _sample_path_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {
                "id": "item:collect",
                "kind": "item",
                "role": "fn",
                "value": "collect_actions",
            },
            {
                "id": "hot:command",
                "kind": "hot",
                "role": "call",
                "value": "command_intent",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "item:collect", "relation": "matches"},
            {"source": "item:collect", "target": "hot:command", "relation": "contains"},
        ],
    }


def _window_packet() -> dict[str, object]:
    return {
        "nodes": [
            {
                "id": "r:read",
                "kind": "range",
                "role": "range",
                "value": "read plan",
                "path": "src/a.py",
                "startLine": 1,
                "endLine": 8,
            },
            {
                "id": "w:a",
                "kind": "window",
                "role": "path",
                "value": "src/a.py",
                "path": "src/a.py",
                "startLine": 10,
                "endLine": 20,
            },
            {
                "id": "w:b",
                "kind": "window",
                "role": "path",
                "value": "src/a.py",
                "path": "src/a.py",
                "startLine": 18,
                "endLine": 28,
            },
        ],
        "edges": [
            {"source": "r:read", "target": "w:a", "relation": "selects"},
            {"source": "r:read", "target": "w:b", "relation": "selects"},
        ],
    }


def test_window_merge_combines_overlapping_ranked_windows() -> None:
    graph = TypedGraph.from_packet(_window_packet())
    result = rank_frontier(graph, profile="read-frontier", seeds=["r:read"])

    assert len(result.merged_windows) == 1
    merged = result.merged_windows[0]
    assert merged.path == "src/a.py"
    assert merged.start_line == 10
    assert merged.end_line == 28
    assert merged.node_ids == ("w:a", "w:b")


def test_typed_paths_and_flow_lite_rank_source_sink_frontier() -> None:
    graph = TypedGraph.from_packet(_sample_path_packet())
    result = rank_frontier(graph, profile="owner-query", seeds=["q:parser"])

    assert result.source_sink_frontier.source_ids == ("q:parser",)
    assert "item:collect" in result.source_sink_frontier.sink_ids
    assert "hot:command" in result.source_sink_frontier.sink_ids
    assert result.typed_paths
    assert result.typed_paths[0].path_kind == "constrained-shortest"
    assert result.typed_paths[0].source == "q:parser"
    assert result.typed_paths[0].node_ids[0] == "q:parser"
    assert result.flow_lite.ranked_path_ids == tuple(
        path.id for path in result.typed_paths
    )


def test_packet_fingerprint_cache_trace_and_explanations_are_response_evidence(
    monkeypatch,
    tmp_path,
) -> None:
    _BACKEND_CACHE.clear()
    monkeypatch.setenv("PRJ_CACHE_HOME", str(tmp_path))
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {"id": "q:cache", "kind": "query", "role": "term", "value": "cache"},
                {
                    "id": "item:cache",
                    "kind": "item",
                    "role": "fn",
                    "value": "load_cache",
                },
            ],
            "edges": [
                {"source": "q:cache", "target": "item:cache", "relation": "matches"},
            ],
        }
    )

    first = rank_frontier(graph, profile="owner-query", seeds=["q:cache"])
    second = rank_frontier(graph, profile="owner-query", seeds=["q:cache"])

    assert first.packet_fingerprint.startswith("sha256:")
    assert first.graph_cache.status == "miss"
    assert second.graph_cache.status == "hit"
    assert [step.step for step in second.algorithm_trace] == [
        "packet-fingerprint",
        "graph-cache",
        "profile-policy",
        "typed-ppr",
        "diverse-rank",
        "typed-paths",
        "window-merge",
        "read-loop-guard",
    ]
    explanations = {
        explanation.node_id: explanation for explanation in second.rank_explanations
    }
    assert "typed-ppr" in explanations["q:cache"].reasons
    assert second.algorithm_metrics.cache_status == "hit"
    assert second.algorithm_metrics.read_loop_direct_code_action_count == 0
