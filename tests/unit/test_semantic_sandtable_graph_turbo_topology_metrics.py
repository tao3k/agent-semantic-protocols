from __future__ import annotations

import json

from tools.semantic_sandtable.agent_session_graph_turbo_events import (
    graph_turbo_seed_plan_candidates_from_events,
)
from tools.semantic_sandtable.large_library_optimization_matrix import (
    optimization_batch,
)


def test_graph_turbo_result_topology_metrics_generate_feedback_candidate() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-result",
        "rank": ["owner:drift", "owner:topology"],
        "algorithmMetrics": {
            "queryTopologyMembershipCandidateCount": 2,
            "queryTopologyMembershipBoostCount": 1,
            "queryTopologyMembershipPenaltyCount": 1,
            "queryTopologyMembershipDirectCount": 1,
            "queryTopologyMembershipNearbyCount": 0,
            "queryTopologyMembershipCoverageRate": 0.5,
            "queryTopologyMembershipDriftRate": 0.5,
            "queryTopologyMembershipDelta": 0.2,
        },
    }
    events = [
        {
            "kind": "command.result",
            "eventId": "event-1",
            "commandId": "cmd-1",
            "preview": json.dumps(packet),
        }
    ]

    candidates = graph_turbo_seed_plan_candidates_from_events({}, events)

    assert len(candidates) == 1
    candidate = candidates[0]
    assert candidate["kind"] == "topology-membership-coverage"
    assert candidate["expectedChange"] == "lower-topology-drift"
    assert candidate["topologyCandidateCount"] == 2
    assert candidate["topologyCoverageRate"] == 0.5
    assert candidate["topologyDriftRate"] == 0.5
    assert candidate["packetNodeIds"] == ["owner:drift", "owner:topology"]


def test_optimization_batch_requires_topology_membership_metrics() -> None:
    batch = optimization_batch(
        [
            {
                "runId": "rust:tokio:deep:query-first-stage",
                "language": "rust",
                "depthBucket": "deep",
                "package": "tokio",
                "questionId": "deep",
            }
        ]
    )

    required_metrics = set(batch["requiredReceiptMetrics"])
    assert "queryTopologyMembershipCandidateCount" in required_metrics
    assert "queryTopologyMembershipCoverageRate" in required_metrics
    assert "queryTopologyMembershipDriftRate" in required_metrics
    assert "queryTopologyMembershipDelta" in required_metrics
