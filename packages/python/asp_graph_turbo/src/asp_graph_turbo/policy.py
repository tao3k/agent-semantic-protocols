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
    "belongs_to": 1.0,
    "packages": 1.0,
    "builds": 1.1,
    "targets": 1.1,
    "tests": 1.4,
    "affects": 1.2,
    "flows-to": 1.6,
    "split": 0.7,
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
}

SAME_OWNER_PENALTY = 0.25
SAME_SYMBOL_NAME_PENALTY = 0.4
SAME_KIND_OVER_BUDGET_PENALTY = 0.3
CONTIGUOUS_WINDOW_MERGE_BONUS = 0.5


def edge_weight_for(relation: str, explicit: Any = None) -> float:
    if isinstance(explicit, int | float):
        return max(float(explicit), 0.0)
    return EDGE_WEIGHT_BY_RELATION.get(relation, 0.0)


def node_kind_bonus(profile_name: str, node_kind: str) -> float:
    return NODE_KIND_BONUS_BY_PROFILE.get(profile_name, {}).get(node_kind, 0.0)
