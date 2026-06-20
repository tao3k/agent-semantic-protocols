"""Focused ASP graph turbo tests."""

from __future__ import annotations

from asp_graph_turbo.cache import backend_fingerprint
from asp_graph_turbo.profiles import resolve_profile
from asp_graph_turbo.ranking_build import rank_fingerprint

from ._asp_graph_turbo_common import (
    TypedGraph,
    rank_frontier,
    sample_packet,
)


def test_owner_query_profile_masks_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph, profile="owner-query", seeds=["q:parser", "owner:cli"]
    )

    ranked = [node.id for node in result.ranked_nodes]

    assert "owner:cli" in ranked
    assert "item:collect" in ranked
    assert "hot:command" in ranked
    assert "test:cli" in ranked
    assert "dep:jsonschema" not in ranked
    assert ("dependency", "deps") not in [
        (entry.node.kind, entry.action) for entry in result.frontier
    ]


def test_backend_fingerprint_ignores_query_policy() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    profile = resolve_profile("owner-query")
    seed_ids = ("q:parser", "owner:cli")

    default_rank_key = rank_fingerprint(
        graph,
        profile,
        seed_ids,
        query_clauses=("parser command",),
        query_adjustment_policy={"seedPrior": True, "localEvidence": True},
        budget=8,
        kind_budgets={},
        path_budget=4,
        path_max_hops=4,
        window_merge_enabled=True,
        window_merge_max_gap_lines=8,
    )
    no_local_evidence_rank_key = rank_fingerprint(
        graph,
        profile,
        seed_ids,
        query_clauses=("parser command",),
        query_adjustment_policy={"seedPrior": True, "localEvidence": False},
        budget=8,
        kind_budgets={},
        path_budget=4,
        path_max_hops=4,
        window_merge_enabled=True,
        window_merge_max_gap_lines=8,
    )

    assert default_rank_key != no_local_evidence_rank_key
    assert backend_fingerprint(graph, profile) == backend_fingerprint(graph, profile)
    assert backend_fingerprint(graph, profile).startswith("sha256:backend:")


def test_runtime_ranking_caches_reuse_seed_work() -> None:
    graph = TypedGraph.from_packet(sample_packet())

    rank_frontier(graph, profile="owner-query", seeds=["q:parser", "owner:cli"])
    second = rank_frontier(graph, profile="owner-query", seeds=["q:parser", "owner:cli"])

    assert second.algorithm_metrics.depth_cache_status == "hit"
    assert second.algorithm_metrics.ppr_cache_status == "hit"
    assert second.algorithm_metrics.reachable_edges_cache_status == "hit"


def test_owner_query_ranking_prefers_rare_query_token_match_text() -> None:
    nodes: list[dict[str, object]] = [
        {
            "id": "q:vec-collection",
            "kind": "query",
            "role": "term",
            "value": "Vec collection",
        },
        {
            "id": "item:collection",
            "kind": "item",
            "role": "symbol",
            "value": "collection",
            "path": "tokio/src/loom/std/mod.rs",
            "ownerPath": "tokio/src/loom/std/mod.rs",
            "symbol": "collection",
            "matchText": "collection helpers for loom std",
        },
    ]
    edges: list[dict[str, str]] = [
        {
            "source": "q:vec-collection",
            "target": "item:collection",
            "relation": "matches",
        }
    ]
    for index in range(8):
        node_id = f"item:vec-{index}"
        nodes.append(
            {
                "id": node_id,
                "kind": "item",
                "role": "symbol",
                "value": "vec",
                "path": f"tokio/src/fs/file_{index}.rs",
                "ownerPath": f"tokio/src/fs/file_{index}.rs",
                "symbol": "vec",
                "matchText": "let buffer: Vec<u8> = Vec::new();",
            }
        )
        edges.append(
            {
                "source": "q:vec-collection",
                "target": node_id,
                "relation": "matches",
            }
        )
    graph = TypedGraph.from_packet(
        {
            "nodes": nodes,
            "edges": edges,
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-collection"],
        limit=4,
        kind_budgets={"query": 1, "item": 3},
    )

    ranked_items = [node.id for node in result.ranked_nodes if node.kind == "item"]

    assert ranked_items[0] == "item:collection"
    assert ranked_items[1].startswith("item:vec-")


def test_owner_query_ranking_prefers_owner_with_local_evidence() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:history",
                    "kind": "query",
                    "role": "term",
                    "value": "history",
                },
                {
                    "id": "owner:path-only",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/history.py",
                },
                {
                    "id": "owner:dense",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/history_timeline.py",
                },
                {
                    "id": "item:dense",
                    "kind": "item",
                    "role": "fn",
                    "value": "history_timeline",
                    "path": "src/history_timeline.py",
                    "ownerPath": "src/history_timeline.py",
                },
                {
                    "id": "test:dense",
                    "kind": "test",
                    "role": "path",
                    "value": "tests/test_history_timeline.py",
                },
            ],
            "edges": [
                {
                    "source": "q:history",
                    "target": "owner:path-only",
                    "relation": "matches",
                },
                {"source": "q:history", "target": "owner:dense", "relation": "matches"},
                {"source": "owner:dense", "target": "item:dense", "relation": "contains"},
                {"source": "owner:dense", "target": "test:dense", "relation": "covers"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:history", "owner:path-only", "owner:dense"],
        limit=4,
        kind_budgets={"query": 1, "owner": 2, "item": 1},
    )

    ranked_owners = [node.id for node in result.ranked_nodes if node.kind == "owner"]
    explanation_reasons = {
        explanation.node_id: explanation.reasons
        for explanation in result.rank_explanations
    }

    assert ranked_owners[:2] == ["owner:dense", "owner:path-only"]
    assert "topology-local-evidence:+0.35" in explanation_reasons["owner:dense"]
    assert "topology-local-evidence:-0.20" in explanation_reasons["owner:path-only"]
