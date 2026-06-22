from __future__ import annotations

import pytest
from scipy.sparse import csr_matrix

from asp_graph_turbo import TypedGraph, rank_frontier, result_to_packet
from asp_graph_turbo.backend import sparse_backend_from_parts
from asp_graph_turbo.pagerank import graph_turbo_typed_personalized_pagerank_result
from asp_graph_turbo.query_clause_coverage import query_clause_coverage_adjustment
from asp_graph_turbo.query_weights import (
    query_package_cohesion_adjustment,
    query_seed_personalization_weights,
)


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


def test_package_cohesion_prefers_package_path_over_same_token_text() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:package",
                    "kind": "query",
                    "role": "term",
                    "value": "asp_graph_turbo pagerank",
                },
                {
                    "id": "item:rust-mention",
                    "kind": "item",
                    "role": "symbol",
                    "value": "asp_graph_turbo pagerank",
                    "path": "crates/agent-semantic-client/tests/unit/search_history.rs",
                    "ownerPath": "crates/agent-semantic-client/tests/unit/search_history.rs",
                    "symbol": "asp_graph_turbo",
                },
                {
                    "id": "item:python-package",
                    "kind": "item",
                    "role": "symbol",
                    "value": "pagerank",
                    "path": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "symbol": "pagerank",
                },
            ],
            "edges": [
                {
                    "source": "q:package",
                    "target": "item:rust-mention",
                    "relation": "matches",
                },
                {
                    "source": "q:package",
                    "target": "item:python-package",
                    "relation": "matches",
                },
            ],
        }
    )

    rust_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:package",),
        node=graph.nodes["item:rust-mention"],
    )
    python_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:package",),
        node=graph.nodes["item:python-package"],
    )

    assert rust_adjustment < 0.0
    assert python_adjustment > 0.0


def test_package_cohesion_changes_rank_frontier_for_deep_package_query() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:package",
                    "kind": "query",
                    "role": "term",
                    "value": "asp_graph_turbo pagerank",
                },
                {
                    "id": "item:rust-mention",
                    "kind": "item",
                    "role": "symbol",
                    "value": "asp_graph_turbo pagerank",
                    "path": "crates/agent-semantic-client/tests/unit/search_history.rs",
                    "ownerPath": "crates/agent-semantic-client/tests/unit/search_history.rs",
                    "symbol": "asp_graph_turbo",
                },
                {
                    "id": "item:python-package",
                    "kind": "item",
                    "role": "symbol",
                    "value": "pagerank",
                    "path": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/pagerank.py"
                    ),
                    "symbol": "pagerank",
                },
            ],
            "edges": [
                {
                    "source": "q:package",
                    "target": "item:rust-mention",
                    "relation": "matches",
                },
                {
                    "source": "q:package",
                    "target": "item:python-package",
                    "relation": "matches",
                },
            ],
        }
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:package"],
        limit=3,
        kind_budgets={"query": 1, "item": 2},
    )

    assert result.ranked_nodes[0].id == "item:python-package"
    assert result.scores["item:python-package"] > result.scores["item:rust-mention"]


def test_clause_coverage_requires_package_path_for_package_clause() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "item:rust-clause-text",
                    "kind": "item",
                    "role": "symbol",
                    "value": "queryClauses coverage typed graph request",
                    "path": (
                        "crates/agent-semantic-protocol/tests/unit/"
                        "provider_command/facade/pipe/query_wrapper/graph_request.rs"
                    ),
                    "ownerPath": (
                        "crates/agent-semantic-protocol/tests/unit/"
                        "provider_command/facade/pipe/query_wrapper/graph_request.rs"
                    ),
                    "symbol": "queryClauses",
                },
                {
                    "id": "item:python-clause-path",
                    "kind": "item",
                    "role": "symbol",
                    "value": "queryClauses coverage typed graph request",
                    "path": (
                        "packages/python/asp_graph_turbo/src/asp_graph_turbo/cli.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/asp_graph_turbo/cli.py"
                    ),
                    "symbol": "queryClauses",
                },
            ],
            "edges": [],
        }
    )
    clauses = (
        "asp_graph_turbo queryClauses clause coverage scoring",
        "typed graph request rank objective",
    )

    assert (
        query_clause_coverage_adjustment(
            profile_name="owner-query",
            query_clauses=clauses,
            node=graph.nodes["item:rust-clause-text"],
        )
        == 0.0
    )
    assert (
        query_clause_coverage_adjustment(
            profile_name="owner-query",
            query_clauses=clauses,
            node=graph.nodes["item:python-clause-path"],
        )
        > 0.0
    )


def test_query_clauses_rank_multi_clause_package_evidence_above_single_clause() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:clauses",
                    "kind": "query",
                    "role": "term",
                    "value": "asp_graph_turbo queryClauses typed graph request",
                },
                {
                    "id": "item:package-only",
                    "kind": "item",
                    "role": "symbol",
                    "value": "queryClauses coverage scoring",
                    "path": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/request_projection.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/request_projection.py"
                    ),
                    "symbol": "queryClauses",
                },
                {
                    "id": "item:package-and-request",
                    "kind": "item",
                    "role": "symbol",
                    "value": "queryClauses coverage scoring typed graph request rank",
                    "path": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/request_projection.py"
                    ),
                    "ownerPath": (
                        "packages/python/asp_graph_turbo/src/"
                        "asp_graph_turbo/request_projection.py"
                    ),
                    "symbol": "queryClauses",
                },
            ],
            "edges": [
                {
                    "source": "q:clauses",
                    "target": "item:package-only",
                    "relation": "matches",
                },
                {
                    "source": "q:clauses",
                    "target": "item:package-and-request",
                    "relation": "matches",
                },
            ],
        }
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:clauses"],
        limit=3,
        kind_budgets={"query": 1, "item": 2},
        query_clauses=(
            "asp_graph_turbo queryClauses clause coverage scoring",
            "typed graph request rank objective",
        ),
    )
    packet = result_to_packet(result)
    explanations = {
        explanation["nodeId"]: explanation["reasons"]
        for explanation in packet["rankExplanations"]
    }

    assert (
        result.scores["item:package-and-request"] > result.scores["item:package-only"]
    )
    assert packet["algorithmMetrics"]["queryPackageCohesionCount"] >= 2
    assert packet["algorithmMetrics"]["queryClauseCoverageCount"] == 1
    assert any(step["step"] == "query-adjustments" for step in packet["algorithmTrace"])
    assert "query-clause-coverage:+0.30" in explanations["item:package-and-request"]


def test_query_clause_priority_prefers_owner_symbols_over_context_compounds() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:handle-enter",
                    "kind": "query",
                    "role": "term",
                    "value": "Tokio runtime Handle enter guard",
                },
                {
                    "id": "item:guard",
                    "kind": "item",
                    "role": "symbol",
                    "value": "guard",
                    "path": "src/runtime/handle.rs",
                    "ownerPath": "src/runtime/handle.rs",
                    "symbol": "guard",
                    "matchText": "pub fn enter(&self) -> EnterGuard { EnterGuard }",
                },
                {
                    "id": "item:handle",
                    "kind": "item",
                    "role": "symbol",
                    "value": "Handle",
                    "path": "src/runtime/handle.rs",
                    "ownerPath": "src/runtime/handle.rs",
                    "symbol": "Handle",
                },
                {
                    "id": "item:enter",
                    "kind": "item",
                    "role": "symbol",
                    "value": "enter",
                    "path": "src/runtime/handle.rs",
                    "ownerPath": "src/runtime/handle.rs",
                    "symbol": "enter",
                },
            ],
            "edges": [
                {
                    "source": "q:handle-enter",
                    "target": "item:guard",
                    "relation": "matches",
                },
                {
                    "source": "q:handle-enter",
                    "target": "item:handle",
                    "relation": "matches",
                },
                {
                    "source": "q:handle-enter",
                    "target": "item:enter",
                    "relation": "matches",
                },
            ],
        }
    )

    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:handle-enter"],
        limit=4,
        kind_budgets={"query": 1, "item": 3},
        query_clauses=("Tokio Handle enter", "runtime guard"),
    )

    ranked_items = [node.id for node in result.ranked_nodes if node.kind == "item"]
    assert ranked_items.index("item:handle") < ranked_items.index("item:guard")
    assert ranked_items.index("item:enter") < ranked_items.index("item:guard")
