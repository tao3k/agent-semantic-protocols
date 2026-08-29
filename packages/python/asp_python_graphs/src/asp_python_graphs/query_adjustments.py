"""Analyzer-facing query adjustment trace helpers for graph turbo."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import TypedGraph
from .query_clause_coverage import (
    CLAUSE_COVERAGE_BONUS,
    query_clause_coverage_adjustment,
)
from .query_local_evidence import local_evidence_adjustment
from .query_package_cohesion import (
    _PACKAGE_COHESION_BONUS,
    _PACKAGE_DRIFT_PENALTY,
    query_package_cohesion_adjustment,
    query_package_cohesion_tokens,
)
from .query_topology_membership import (
    TOPOLOGY_DRIFT_PENALTY,
    TOPOLOGY_MEMBERSHIP_BONUS,
    topology_membership_adjustment,
)
from .query_weights import (
    _MATCHED_SEED_FLOOR,
    _QUERY_SEED_WEIGHT,
    _UNMATCHED_SEED_FLOOR,
    query_seed_personalization_weights,
)

_QUERY_ADJUSTMENT_POLICY_KEYS = (
    "seedPrior",
    "packageCohesion",
    "queryClauseCoverage",
    "localEvidence",
    "topologyMembership",
)
_DEFAULT_QUERY_ADJUSTMENT_POLICY = {
    "seedPrior": True,
    "packageCohesion": True,
    "queryClauseCoverage": True,
    "localEvidence": True,
    "topologyMembership": True,
}


def normalize_query_adjustment_policy(
    policy: Mapping[str, object] | None = None,
) -> Mapping[str, bool]:
    normalized = dict(_DEFAULT_QUERY_ADJUSTMENT_POLICY)
    if not isinstance(policy, Mapping):
        return normalized
    for key in _QUERY_ADJUSTMENT_POLICY_KEYS:
        value = policy.get(key)
        if isinstance(value, bool):
            normalized[key] = value
    return normalized


def query_adjustments_by_node(
    graph: TypedGraph,
    *,
    profile_name: str,
    seed_ids: Iterable[str],
    query_clauses: Iterable[str],
    policy: Mapping[str, object] | None = None,
) -> Mapping[str, Mapping[str, float]]:
    normalized_policy = normalize_query_adjustment_policy(policy)
    seed_id_tuple = tuple(seed_ids)
    adjustments: dict[str, dict[str, float]] = {}
    package_tokens = (
        query_package_cohesion_tokens(graph, seed_id_tuple)
        if normalized_policy["packageCohesion"]
        else ()
    )
    if normalized_policy["seedPrior"]:
        _record_seed_prior_adjustments(
            adjustments,
            graph=graph,
            profile_name=profile_name,
            seed_ids=seed_id_tuple,
        )
    for node in graph.nodes.values():
        _record_node_adjustments(
            adjustments,
            graph=graph,
            profile_name=profile_name,
            seed_ids=seed_id_tuple,
            query_clauses=query_clauses,
            policy=normalized_policy,
            package_tokens=package_tokens,
            node_id=node.id,
        )
    return adjustments


def _record_seed_prior_adjustments(
    adjustments: dict[str, dict[str, float]],
    *,
    graph: TypedGraph,
    profile_name: str,
    seed_ids: tuple[str, ...],
) -> None:
    for seed_id, weight in query_seed_personalization_weights(
        graph,
        profile_name=profile_name,
        seed_ids=seed_ids,
    ).items():
        _record_adjustment(adjustments, seed_id, "seedPrior", weight)


def _record_node_adjustments(
    adjustments: dict[str, dict[str, float]],
    *,
    graph: TypedGraph,
    profile_name: str,
    seed_ids: tuple[str, ...],
    query_clauses: Iterable[str],
    policy: Mapping[str, bool],
    package_tokens: tuple[str, ...],
    node_id: str,
) -> None:
    node = graph.nodes[node_id]
    if policy["packageCohesion"]:
        _record_adjustment(
            adjustments,
            node_id,
            "packageCohesion",
            query_package_cohesion_adjustment(
                graph,
                profile_name=profile_name,
                seed_ids=seed_ids,
                node=node,
                package_tokens=package_tokens,
            ),
        )
    if policy["queryClauseCoverage"]:
        _record_adjustment(
            adjustments,
            node_id,
            "queryClauseCoverage",
            query_clause_coverage_adjustment(
                profile_name=profile_name,
                query_clauses=query_clauses,
                node=node,
            ),
        )
    if policy["localEvidence"]:
        _record_adjustment(
            adjustments,
            node_id,
            "localEvidence",
            local_evidence_adjustment(
                graph,
                profile_name=profile_name,
                node_id=node_id,
            ),
        )
    if policy["topologyMembership"]:
        _record_adjustment(
            adjustments,
            node_id,
            "topologyMembership",
            topology_membership_adjustment(
                graph,
                profile_name=profile_name,
                node_id=node_id,
            ),
        )


def _record_adjustment(
    adjustments: dict[str, dict[str, float]],
    node_id: str,
    name: str,
    delta: float,
) -> None:
    if delta:
        adjustments.setdefault(node_id, {})[name] = round(delta, 6)


def query_adjustment_summary(
    adjustments: Mapping[str, Mapping[str, float]],
) -> Mapping[str, int | float]:
    seed_prior_values = _adjustment_values(adjustments, "seedPrior")
    package_values = _adjustment_values(adjustments, "packageCohesion")
    clause_values = _adjustment_values(adjustments, "queryClauseCoverage")
    local_values = _adjustment_values(adjustments, "localEvidence")
    topology_values = _adjustment_values(adjustments, "topologyMembership")
    topology_boost_count = sum(1 for value in topology_values if value > 0)
    topology_penalty_count = sum(1 for value in topology_values if value < 0)
    topology_candidate_count = len(topology_values)
    topology_direct_count = sum(
        1
        for value in topology_values
        if round(value, 6) == round(TOPOLOGY_MEMBERSHIP_BONUS, 6)
    )
    topology_nearby_count = topology_boost_count - topology_direct_count
    return {
        "querySeedPriorCount": len(seed_prior_values),
        "querySeedPriorMass": round(sum(seed_prior_values), 6),
        "queryPackageCohesionCount": sum(1 for value in package_values if value > 0),
        "queryPackageDriftPenaltyCount": sum(
            1 for value in package_values if value < 0
        ),
        "queryPackageCohesionDelta": round(sum(package_values), 6),
        "queryClauseCoverageCount": len(clause_values),
        "queryClauseCoverageDelta": round(sum(clause_values), 6),
        "queryLocalEvidenceBoostCount": sum(1 for value in local_values if value > 0),
        "queryLocalEvidencePenaltyCount": sum(1 for value in local_values if value < 0),
        "queryLocalEvidenceDelta": round(sum(local_values), 6),
        "queryTopologyMembershipCandidateCount": topology_candidate_count,
        "queryTopologyMembershipBoostCount": topology_boost_count,
        "queryTopologyMembershipPenaltyCount": topology_penalty_count,
        "queryTopologyMembershipDirectCount": topology_direct_count,
        "queryTopologyMembershipNearbyCount": topology_nearby_count,
        "queryTopologyMembershipCoverageRate": _ratio(
            topology_boost_count,
            topology_candidate_count,
        ),
        "queryTopologyMembershipDriftRate": _ratio(
            topology_penalty_count,
            topology_candidate_count,
        ),
        "queryTopologyMembershipDelta": round(sum(topology_values), 6),
    }


def query_adjustment_guardrails() -> Mapping[str, Mapping[str, float | str]]:
    return {
        "seedPrior": {
            "default": _QUERY_SEED_WEIGHT,
            "min": 1.0,
            "max": 2.0,
            "metric": "querySeedPriorCount",
        },
        "matchedSeedFloor": {
            "default": _MATCHED_SEED_FLOOR,
            "min": 0.1,
            "max": 0.8,
            "metric": "querySeedPriorMass",
        },
        "unmatchedSeedFloor": {
            "default": _UNMATCHED_SEED_FLOOR,
            "min": 0.0,
            "max": 0.5,
            "metric": "querySeedPriorMass",
        },
        "packageCohesionBonus": {
            "default": _PACKAGE_COHESION_BONUS,
            "min": 0.0,
            "max": 1.0,
            "metric": "queryPackageCohesionDelta",
        },
        "packageDriftPenalty": {
            "default": _PACKAGE_DRIFT_PENALTY,
            "min": 0.0,
            "max": 1.0,
            "metric": "queryPackageCohesionDelta",
        },
        "queryClauseCoverageBonus": {
            "default": CLAUSE_COVERAGE_BONUS,
            "min": 0.0,
            "max": 0.8,
            "metric": "queryClauseCoverageDelta",
        },
        "topologyMembershipBonus": {
            "default": TOPOLOGY_MEMBERSHIP_BONUS,
            "min": 0.0,
            "max": 1.0,
            "metric": "queryTopologyMembershipDelta",
        },
        "topologyDriftPenalty": {
            "default": TOPOLOGY_DRIFT_PENALTY,
            "min": 0.0,
            "max": 1.0,
            "metric": "queryTopologyMembershipDelta",
        },
    }


def _adjustment_values(
    adjustments: Mapping[str, Mapping[str, float]], name: str
) -> tuple[float, ...]:
    return tuple(
        float(value)
        for node_adjustments in adjustments.values()
        if isinstance((value := node_adjustments.get(name)), int | float)
    )


def _ratio(numerator: int, denominator: int) -> float:
    return round(numerator / denominator, 6) if denominator else 0.0
