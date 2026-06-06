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
    "flows-to": 1.6,
    "split": 0.7,
}

NODE_KIND_BONUS_BY_PROFILE: Mapping[str, Mapping[str, float]] = {
    "owner-query": {
        "hot": 0.3,
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
