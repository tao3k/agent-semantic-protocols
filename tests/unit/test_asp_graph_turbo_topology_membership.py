from __future__ import annotations

from asp_graph_turbo import TypedGraph, rank_frontier
from asp_graph_turbo.query_adjustments import (
    query_adjustment_summary,
    query_adjustments_by_node,
)
from asp_graph_turbo.query_topology_membership import topology_membership_adjustment


def test_topology_membership_prefers_owner_in_workspace_cluster() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:ranking",
                    "kind": "query",
                    "role": "term",
                    "value": "graph turbo ranking",
                },
                {
                    "id": "workspace:root",
                    "kind": "workspace",
                    "role": "root",
                    "value": ".",
                },
                {
                    "id": "submodule:graph-turbo",
                    "kind": "submodule",
                    "role": "workspace-member",
                    "value": "packages/python/asp_graph_turbo",
                    "path": "packages/python/asp_graph_turbo",
                },
                {
                    "id": "owner:ranking",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking.py"
                    ),
                    "path": (
                        "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking.py"
                    ),
                },
                {
                    "id": "owner:drift",
                    "kind": "owner",
                    "role": "path",
                    "value": "tests/unit/test_asp_graph_turbo_ranking_collection.py",
                    "path": "tests/unit/test_asp_graph_turbo_ranking_collection.py",
                    "ownerPath": "tests/unit/test_asp_graph_turbo_ranking_collection.py",
                },
            ],
            "edges": [
                {
                    "source": "q:ranking",
                    "target": "owner:ranking",
                    "relation": "matches",
                },
                {
                    "source": "q:ranking",
                    "target": "owner:drift",
                    "relation": "matches",
                },
                {
                    "source": "workspace:root",
                    "target": "submodule:graph-turbo",
                    "relation": "has_submodule",
                },
                {
                    "source": "submodule:graph-turbo",
                    "target": "owner:ranking",
                    "relation": "contains",
                },
            ],
        }
    )

    assert (
        topology_membership_adjustment(
            graph,
            profile_name="owner-query",
            node_id="owner:ranking",
        )
        > 0.0
    )
    assert (
        topology_membership_adjustment(
            graph,
            profile_name="owner-query",
            node_id="owner:drift",
        )
        < 0.0
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:ranking"],
        limit=3,
        kind_budgets={"query": 1, "owner": 2},
    )

    assert result.ranked_nodes[0].id == "owner:ranking"
    assert result.scores["owner:ranking"] > result.scores["owner:drift"]
    assert result.algorithm_metrics.query_topology_membership_candidate_count == 2
    assert result.algorithm_metrics.query_topology_membership_direct_count == 1
    assert result.algorithm_metrics.query_topology_membership_nearby_count == 0
    assert result.algorithm_metrics.query_topology_membership_coverage_rate == 0.5
    assert result.algorithm_metrics.query_topology_membership_drift_rate == 0.5

    summary = query_adjustment_summary(
        query_adjustments_by_node(
            graph,
            profile_name="owner-query",
            seed_ids=["q:ranking"],
            query_clauses=["graph turbo ranking"],
        )
    )
    assert summary["queryTopologyMembershipCandidateCount"] == 2
    assert summary["queryTopologyMembershipCoverageRate"] == 0.5
    assert summary["queryTopologyMembershipDriftRate"] == 0.5


def test_topology_membership_prefers_local_anchor_over_workspace_root() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:ranking",
                    "kind": "query",
                    "role": "term",
                    "value": "typescript owner ranking",
                },
                {
                    "id": "workspace:root",
                    "kind": "workspace",
                    "role": "root",
                    "value": ".",
                },
                {
                    "id": "submodule:typescript",
                    "kind": "submodule",
                    "role": "workspace-member",
                    "value": "languages/typescript-lang-project-harness",
                },
                {
                    "id": "owner:submodule",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/src/cli/"
                        "semantic-search/workspace-ranking.ts"
                    ),
                },
                {
                    "id": "owner:workspace-root",
                    "kind": "owner",
                    "role": "path",
                    "value": "README.md",
                },
            ],
            "edges": [
                {
                    "source": "q:ranking",
                    "target": "owner:submodule",
                    "relation": "matches",
                },
                {
                    "source": "q:ranking",
                    "target": "owner:workspace-root",
                    "relation": "matches",
                },
                {
                    "source": "workspace:root",
                    "target": "submodule:typescript",
                    "relation": "has_submodule",
                },
                {
                    "source": "submodule:typescript",
                    "target": "owner:submodule",
                    "relation": "contains",
                },
                {
                    "source": "workspace:root",
                    "target": "owner:workspace-root",
                    "relation": "contains",
                },
            ],
        }
    )

    assert topology_membership_adjustment(
        graph,
        profile_name="owner-query",
        node_id="owner:submodule",
    ) > topology_membership_adjustment(
        graph,
        profile_name="owner-query",
        node_id="owner:workspace-root",
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:ranking"],
        limit=3,
        kind_budgets={"query": 1, "owner": 2},
    )

    assert result.ranked_nodes[0].id == "owner:submodule"
    assert result.algorithm_metrics.query_topology_membership_candidate_count == 2
    assert result.algorithm_metrics.query_topology_membership_direct_count == 1
    assert result.algorithm_metrics.query_topology_membership_nearby_count == 1
    assert result.algorithm_metrics.query_topology_membership_coverage_rate == 1.0
    assert result.algorithm_metrics.query_topology_membership_drift_rate == 0.0
