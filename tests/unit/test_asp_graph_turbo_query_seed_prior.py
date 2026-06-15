from __future__ import annotations

import pytest
from scipy.sparse import csr_matrix

from asp_graph_turbo import TypedGraph
from asp_graph_turbo.backend import sparse_backend_from_parts
from asp_graph_turbo.pagerank import graph_turbo_typed_personalized_pagerank_result
from asp_graph_turbo.query_weights import query_seed_personalization_weights


def test_owner_query_seed_prior_weights_specific_owner_above_generic_owner() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:algorithm",
                    "kind": "query",
                    "role": "term",
                    "value": "personalized pagerank optimization",
                },
                {
                    "id": "owner:generic",
                    "kind": "owner",
                    "role": "path",
                    "value": "crates/agent-semantic-protocol/src/command/graph.rs",
                    "path": "crates/agent-semantic-protocol/src/command/graph.rs",
                    "ownerPath": "crates/agent-semantic-protocol/src/command/graph.rs",
                },
                {
                    "id": "owner:pagerank",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "path": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                },
            ],
            "edges": [],
        }
    )

    weights = query_seed_personalization_weights(
        graph,
        profile_name="owner-query",
        seed_ids=("q:algorithm", "owner:generic", "owner:pagerank"),
    )

    assert weights["q:algorithm"] > weights["owner:pagerank"]
    assert weights["owner:pagerank"] > weights["owner:generic"]
    assert weights["owner:generic"] == pytest.approx(0.20)


def test_weighted_personalized_pagerank_uses_seed_prior_mass() -> None:
    backend = sparse_backend_from_parts(
        ("q:algorithm", "owner:generic", "owner:pagerank"),
        csr_matrix((3, 3)),
        (),
    )

    result = graph_turbo_typed_personalized_pagerank_result(
        backend,
        ("q:algorithm", "owner:generic", "owner:pagerank"),
        seed_weights={
            "q:algorithm": 2.0,
            "owner:generic": 0.20,
            "owner:pagerank": 1.20,
        },
    )

    assert result.scores["q:algorithm"] > result.scores["owner:pagerank"]
    assert result.scores["owner:pagerank"] > result.scores["owner:generic"]
    assert result.mass_sum == pytest.approx(1.0)
