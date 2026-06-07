"""Profile configuration model for ASP graph turbo."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field


@dataclass(frozen=True, order=True)
class AllowedTransition:
    source_kind: str
    target_kind: str


@dataclass(frozen=True)
class GraphProfile:
    name: str
    allowed_relations: frozenset[str]
    allowed_transitions: frozenset[AllowedTransition]
    frontier_actions: Mapping[str, str]
    kind_bonus: Mapping[str, float] = field(default_factory=dict)
    max_depth: int = 2


@dataclass(frozen=True)
class ProfileCompatibility:
    profile: str
    compatible: bool
    allowed_relations: tuple[str, ...]
    allowed_transitions: tuple[AllowedTransition, ...]
    kind_bonus: Mapping[str, float]
    frontier_actions: Mapping[str, str]


@dataclass(frozen=True)
class ProfileMatrixSummary:
    profile: str
    relation_count: int
    transition_count: int
    supported_edge_count: int
    reachable_edge_count: int
    density: float
