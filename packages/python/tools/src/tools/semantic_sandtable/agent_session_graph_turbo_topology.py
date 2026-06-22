"""Graph-turbo topology membership candidates from result packets."""

from __future__ import annotations

import json
from typing import Any

from .utils import dict_value, list_value, optional_int, require_str


def topology_membership_candidates_from_event(
    event: dict[str, Any],
    stdout_texts: list[str],
) -> list[dict[str, Any]]:
    candidates = []
    for packet in _graph_turbo_result_packets(stdout_texts):
        candidate = _candidate_from_topology_metrics(event, packet)
        if candidate:
            candidates.append(candidate)
    return candidates


def _graph_turbo_result_packets(stdout_texts: list[str]) -> list[dict[str, Any]]:
    packets = []
    for text in stdout_texts:
        packet = _json_object(text)
        if packet.get("schemaId") == (
            "agent.semantic-protocols.semantic-graph-turbo-result"
        ):
            packets.append(packet)
    return packets


def _candidate_from_topology_metrics(
    event: dict[str, Any],
    packet: dict[str, Any],
) -> dict[str, Any] | None:
    metrics = dict_value(packet.get("algorithmMetrics"))
    candidate_count = optional_int(metrics.get("queryTopologyMembershipCandidateCount"))
    if not candidate_count:
        return None
    coverage_rate = _optional_float(metrics.get("queryTopologyMembershipCoverageRate"))
    drift_rate = _optional_float(metrics.get("queryTopologyMembershipDriftRate"))
    if coverage_rate is None and drift_rate is None:
        return None
    coverage = coverage_rate or 0.0
    drift = drift_rate or 0.0
    if coverage >= 0.75 and drift <= 0.25:
        return None
    command_id = require_str(
        event, "commandId", require_str(event, "eventId", "command")
    )
    boost_count = optional_int(metrics.get("queryTopologyMembershipBoostCount")) or 0
    penalty_count = (
        optional_int(metrics.get("queryTopologyMembershipPenaltyCount")) or 0
    )
    direct_count = optional_int(metrics.get("queryTopologyMembershipDirectCount")) or 0
    nearby_count = optional_int(metrics.get("queryTopologyMembershipNearbyCount")) or 0
    delta = _optional_float(metrics.get("queryTopologyMembershipDelta")) or 0.0
    expected_change, recommended_action, confidence = _topology_recommendation(
        coverage,
        drift,
    )
    return {
        "id": f"gt.topology-membership.{command_id}",
        "kind": "topology-membership-coverage",
        "confidence": confidence,
        "reason": (
            "Graph-turbo topology membership covered "
            f"{boost_count}/{candidate_count} owner candidate(s), "
            f"direct={direct_count}, nearby={nearby_count}, "
            f"penalty={penalty_count}, coverageRate={coverage:.6f}, "
            f"driftRate={drift:.6f}, delta={delta:.6f}."
        ),
        "evidenceRefs": [require_str(event, "eventId", command_id)],
        "packetNodeIds": [str(item) for item in list_value(packet.get("rank"))],
        "topologyCandidateCount": candidate_count,
        "topologyCoverageRate": coverage,
        "topologyDriftRate": drift,
        "expectedChange": expected_change,
        "recommendedAction": recommended_action,
    }


def _topology_recommendation(
    coverage: float,
    drift: float,
) -> tuple[str, str, float]:
    if coverage == 0.0 and drift > 0.0:
        return (
            "increase-topology-coverage",
            "Feed package or submodule topology into graph-turbo before owner "
            "ranking; current owner candidates are mostly outside topology.",
            0.85,
        )
    if drift >= 0.5:
        return (
            "lower-topology-drift",
            "Tighten owner candidate ranking around topology membership before "
            "calibrating other query-first-stage weights.",
            0.8,
        )
    return (
        "increase-topology-coverage",
        "Improve topology membership coverage for owner candidates before "
        "using the run as weight-calibration evidence.",
        0.7,
    )


def _json_object(text: str) -> dict[str, Any]:
    try:
        value = json.loads(text.strip())
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def _optional_float(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    return float(value) if isinstance(value, int | float) else None
