"""Analyzer-facing query adjustment trace helpers for graph turbo."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import TypedGraph
from .query_clause_coverage import (
    CLAUSE_COVERAGE_BONUS,
    query_clause_coverage_adjustment,
)
from .query_weights import (
    _MATCHED_SEED_FLOOR,
    _PACKAGE_COHESION_BONUS,
    _PACKAGE_DRIFT_PENALTY,
    _QUERY_SEED_WEIGHT,
    _UNMATCHED_SEED_FLOOR,
    query_package_cohesion_adjustment,
    query_seed_personalization_weights,
)

_QUERY_ADJUSTMENT_POLICY_KEYS = (
    "seedPrior",
    "packageCohesion",
    "queryClauseCoverage",
)
_DEFAULT_QUERY_ADJUSTMENT_POLICY = {
    "seedPrior": True,
    "packageCohesion": True,
    "queryClauseCoverage": True,
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
    if normalized_policy["seedPrior"]:
        for seed_id, weight in query_seed_personalization_weights(
            graph,
            profile_name=profile_name,
            seed_ids=seed_id_tuple,
        ).items():
            if weight:
                adjustments.setdefault(seed_id, {})["seedPrior"] = round(weight, 6)
    for node in graph.nodes.values():
        if normalized_policy["packageCohesion"]:
            package_delta = query_package_cohesion_adjustment(
                graph,
                profile_name=profile_name,
                seed_ids=seed_id_tuple,
                node=node,
            )
            if package_delta:
                adjustments.setdefault(node.id, {})["packageCohesion"] = round(
                    package_delta, 6
                )
        if normalized_policy["queryClauseCoverage"]:
            clause_delta = query_clause_coverage_adjustment(
                profile_name=profile_name,
                query_clauses=query_clauses,
                node=node,
            )
            if clause_delta:
                adjustments.setdefault(node.id, {})["queryClauseCoverage"] = round(
                    clause_delta, 6
                )
    return adjustments


def query_adjustment_summary(
    adjustments: Mapping[str, Mapping[str, float]],
) -> Mapping[str, int | float]:
    seed_prior_values = _adjustment_values(adjustments, "seedPrior")
    package_values = _adjustment_values(adjustments, "packageCohesion")
    clause_values = _adjustment_values(adjustments, "queryClauseCoverage")
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
    }


def _adjustment_values(
    adjustments: Mapping[str, Mapping[str, float]], name: str
) -> tuple[float, ...]:
    return tuple(
        float(value)
        for node_adjustments in adjustments.values()
        if isinstance((value := node_adjustments.get(name)), int | float)
    )
