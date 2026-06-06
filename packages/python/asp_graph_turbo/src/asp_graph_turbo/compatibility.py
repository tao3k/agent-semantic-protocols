"""Profile compatibility evidence for graph turbo results."""

from __future__ import annotations

from collections.abc import Iterable

from .model import Edge, Node, ProfileCompatibility
from .profiles import DEFAULT_PROFILES, frontier_action


def profile_compatibility(
    edges: Iterable[Edge], ranked_nodes: Iterable[Node]
) -> tuple[ProfileCompatibility, ...]:
    selected_relations = {edge.relation for edge in edges}
    ranked = tuple(ranked_nodes)
    return tuple(
        ProfileCompatibility(
            profile=profile.name,
            compatible=_is_compatible(profile.name, selected_relations, ranked),
            allowed_relations=tuple(sorted(profile.allowed_relations)),
            allowed_transitions=tuple(sorted(profile.allowed_transitions)),
            kind_bonus=dict(profile.kind_bonus),
            frontier_actions=dict(profile.frontier_actions),
        )
        for profile in DEFAULT_PROFILES.values()
    )


def _is_compatible(
    profile_name: str, selected_relations: set[str], ranked_nodes: tuple[Node, ...]
) -> bool:
    profile = DEFAULT_PROFILES[profile_name]
    if not selected_relations.issubset(profile.allowed_relations):
        return False
    return all(frontier_action(profile, node) is not None for node in ranked_nodes)
