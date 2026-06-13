"""Graph turbo ranking policy constants."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

EDGE_WEIGHT_BY_RELATION: Mapping[str, float] = {
    "selects": 1.2,
    "matches": 1.5,
    "repairs": 1.8,
    "contains": 1.0,
    "calls": 1.2,
    "covers": 1.4,
    "covered_by": 1.4,
    "imports": 0.8,
    "uses": 0.9,
    "depends_on": 0.9,
    "version_locked": 1.0,
    "uses_api": 1.15,
    "documented_by": 1.1,
    "example_of": 1.0,
    "tested_by": 1.2,
    "compatible_with": 1.05,
    "deprecated_by": 1.1,
    "belongs_to": 1.0,
    "packages": 1.0,
    "builds": 1.1,
    "targets": 1.1,
    "tests": 1.4,
    "affects": 1.2,
    "flows-to": 1.6,
    "split": 0.7,
    "derived-from": 1.0,
    "requires-evidence": 1.25,
    "verified-by": 1.45,
    "observed-by": 1.2,
    "waived-by": 0.75,
    "reviewed-by": 1.15,
    "suggests-action": 1.1,
    "supports-claim": 1.3,
    "fails": 1.7,
    "explains": 1.5,
    "checks": 1.4,
    "gates": 1.3,
    "relates": 1.1,
    "validates": 1.2,
    "has_type": 1.35,
    "collection_of": 1.3,
}

NODE_KIND_BONUS_BY_PROFILE: Mapping[str, Mapping[str, float]] = {
    "owner-query": {
        "field": 0.4,
        "hot": 0.35,
        "collection": 0.3,
        "type": 0.28,
        "item": 0.25,
        "test": 0.15,
        "owner": 0.1,
    },
    "prime": {
        "owner": 0.3,
        "query": 0.2,
        "dependency": 0.15,
        "test": 0.15,
    },
    "read-frontier": {
        "window": 0.4,
        "symbol": 0.3,
        "range": -0.2,
    },
    "failure-frontier": {
        "assert": 0.4,
        "hot": 0.35,
        "evidence": 0.22,
        "key": 0.2,
        "test": 0.12,
        "owner": 0.05,
    },
    "field-impact": {
        "field": 0.5,
        "type": 0.32,
        "hot": 0.3,
        "test": 0.18,
        "owner": 0.08,
    },
    "type-impact": {
        "type": 0.5,
        "field": 0.28,
        "collection": 0.24,
        "hot": 0.22,
        "test": 0.18,
    },
    "collection-impact": {
        "collection": 0.5,
        "field": 0.34,
        "type": 0.26,
        "hot": 0.22,
        "test": 0.16,
    },
    "failure-evidence": {
        "assert": 0.46,
        "evidence": 0.38,
        "hot": 0.32,
        "field": 0.26,
        "collection": 0.22,
        "type": 0.2,
        "test": 0.14,
    },
    "test-selection": {
        "test": 0.55,
        "field": 0.28,
        "hot": 0.24,
        "owner": 0.18,
        "package": 0.12,
        "build": 0.08,
    },
    "affected": {
        "package": 0.42,
        "build": 0.34,
        "test": 0.3,
        "dependency": 0.24,
        "owner": 0.16,
        "field": 0.12,
    },
    "evidence-quality": {
        "evidence-gap": 0.58,
        "invariant-candidate": 0.52,
        "verification-receipt": 0.46,
        "behavior-snapshot": 0.36,
        "determinism-readiness": 0.3,
        "formal-proof-pilot": 0.28,
        "review-action": 0.24,
        "review-packet": 0.18,
        "owner": 0.12,
        "waiver": -0.18,
    },
    "rust-evidence-quality": {
        "evidence-gap": 0.58,
        "invariant-candidate": 0.52,
        "verification-receipt": 0.46,
        "behavior-snapshot": 0.36,
        "determinism-readiness": 0.3,
        "formal-proof-pilot": 0.28,
        "review-action": 0.24,
        "review-packet": 0.18,
        "owner": 0.12,
        "waiver": -0.18,
    },
}

SAME_OWNER_PENALTY = 0.25
SAME_SYMBOL_NAME_PENALTY = 0.4
SAME_KIND_OVER_BUDGET_PENALTY = 0.3
CONTIGUOUS_WINDOW_MERGE_BONUS = 0.5

PROVENANCE_WEIGHT_MULTIPLIER: Mapping[str, float] = {
    "parser": 1.15,
    "build": 1.05,
    "test": 1.10,
    "failure": 1.20,
    "receipt": 1.25,
    "heuristic": 0.65,
}

CONFIDENCE_WEIGHT_MULTIPLIER: Mapping[str, float] = {
    "exact": 1.20,
    "high": 1.05,
    "medium": 0.90,
    "low": 0.65,
    "heuristic": 0.55,
}

FRESHNESS_WEIGHT_MULTIPLIER: Mapping[str, float] = {
    "fresh": 1.05,
    "cache-hit": 1.00,
    "unknown": 0.85,
    "stale": 0.45,
}


def edge_weight_for(
    relation: str, explicit: Any = None, fields: Mapping[str, Any] | None = None
) -> float:
    relation_weight = EDGE_WEIGHT_BY_RELATION.get(relation, 0.0)
    if relation_weight <= 0.0:
        return 0.0
    explicit_weight = (
        max(float(explicit), 0.0) if isinstance(explicit, int | float) else 1.0
    )
    return (
        relation_weight
        * explicit_weight
        * _quality_multiplier(PROVENANCE_WEIGHT_MULTIPLIER, fields, "provenance")
        * _quality_multiplier(CONFIDENCE_WEIGHT_MULTIPLIER, fields, "confidence")
        * _quality_multiplier(FRESHNESS_WEIGHT_MULTIPLIER, fields, "freshness")
    )


def node_kind_bonus(profile_name: str, node_kind: str) -> float:
    return NODE_KIND_BONUS_BY_PROFILE.get(profile_name, {}).get(node_kind, 0.0)


def _quality_multiplier(
    multipliers: Mapping[str, float],
    fields: Mapping[str, Any] | None,
    key: str,
) -> float:
    if fields is None:
        return 1.0
    value = fields.get(key)
    if value is None:
        nested_fields = fields.get("fields")
        if isinstance(nested_fields, Mapping):
            value = nested_fields.get(key)
    return multipliers.get(str(value), 1.0) if value is not None else 1.0
