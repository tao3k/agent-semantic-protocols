"""Topology membership metric packet helpers."""

from __future__ import annotations

from collections.abc import Callable, Mapping

TOPOLOGY_MEMBERSHIP_METRIC_KEYS = (
    "queryTopologyMembershipCandidateCount",
    "queryTopologyMembershipBoostCount",
    "queryTopologyMembershipPenaltyCount",
    "queryTopologyMembershipDirectCount",
    "queryTopologyMembershipNearbyCount",
    "queryTopologyMembershipCoverageRate",
    "queryTopologyMembershipDriftRate",
    "queryTopologyMembershipDelta",
)
TOPOLOGY_MEMBERSHIP_ABLATION_METRIC_KEYS = tuple(
    key
    for key in TOPOLOGY_MEMBERSHIP_METRIC_KEYS
    if key != "queryTopologyMembershipDelta"
)
TOPOLOGY_MEMBERSHIP_ABLATION_DELTA_KEYS = tuple(
    f"{key}Delta" for key in TOPOLOGY_MEMBERSHIP_ABLATION_METRIC_KEYS
)


def topology_membership_metric_packet(
    metrics: Mapping[str, object],
) -> dict[str, object]:
    return {key: metrics.get(key) for key in TOPOLOGY_MEMBERSHIP_METRIC_KEYS}


def topology_membership_delta_packet(
    metric_delta: Callable[[str], object],
) -> dict[str, object]:
    return {
        f"{key}Delta": metric_delta(key)
        for key in TOPOLOGY_MEMBERSHIP_ABLATION_METRIC_KEYS
    }
