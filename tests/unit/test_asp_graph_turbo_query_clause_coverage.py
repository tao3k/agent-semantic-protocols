"""Graph turbo query-clause coverage ranking tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import TypedGraph, rank_frontier


def test_owner_query_reports_match_text_clause_coverage() -> None:
    graph = TypedGraph.from_packet(_routing_drift_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:routing"],
        query_clauses=(
            "resident child routing validation",
            "binary gate deny",
            "model mismatch template",
        ),
        query_adjustment_policy={
            "packageCohesion": False,
            "localEvidence": False,
            "topologyMembership": False,
        },
        limit=3,
        kind_budgets={"query": 1, "owner": 2},
    )

    ranked_owners = [node.id for node in result.ranked_nodes if node.kind == "owner"]
    explanation_reasons = {
        explanation.node_id: explanation.reasons
        for explanation in result.rank_explanations
    }

    assert ranked_owners[0] == "owner:session"
    assert "query-clause-coverage:+0.30" in explanation_reasons["owner:session"]
    assert "query-clause-coverage:+0.30" not in explanation_reasons["owner:model"]


def _routing_drift_packet() -> dict[str, list[dict[str, str]]]:
    return {
        "nodes": [
            {
                "id": "q:routing",
                "kind": "query",
                "role": "term",
                "value": (
                    "resident child routing validation model mismatch "
                    "binary gate deny template"
                ),
            },
            {
                "id": "owner:model",
                "kind": "owner",
                "role": "path",
                "value": "crates/agent-semantic-config/src/hook_client_config/model.rs",
                "ownerPath": (
                    "crates/agent-semantic-config/src/hook_client_config/model.rs"
                ),
                "matchText": "model mismatch template hook_client_config",
            },
            {
                "id": "owner:session",
                "kind": "owner",
                "role": "path",
                "value": (
                    "crates/agent-semantic-protocol/src/command/"
                    "agent_session_registry_state.rs"
                ),
                "ownerPath": (
                    "crates/agent-semantic-protocol/src/command/"
                    "agent_session_registry_state.rs"
                ),
                "matchText": "resident child routing validation binary gate deny",
            },
        ],
        "edges": [
            {
                "source": "q:routing",
                "target": "owner:model",
                "relation": "matches",
            },
            {
                "source": "q:routing",
                "target": "owner:session",
                "relation": "matches",
            },
        ],
    }
