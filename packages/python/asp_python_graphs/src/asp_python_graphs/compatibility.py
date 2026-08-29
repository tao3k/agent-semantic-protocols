"""Profile compatibility evidence for graph turbo results."""

from __future__ import annotations

from collections.abc import Iterable

from .model import GraphProfile, Node, OrientedEdge, ProfileCompatibility
from .profiles import DEFAULT_PROFILES, frontier_action


def profile_compatibility(
    edges: Iterable[OrientedEdge],
    ranked_nodes: Iterable[Node],
    selected_profile: GraphProfile | None = None,
) -> tuple[ProfileCompatibility, ...]:
    selected_relations = {edge.relation for edge in edges}
    ranked = tuple(ranked_nodes)
    return tuple(
        ProfileCompatibility(
            profile=profile.name,
            compatible=_is_compatible(profile, selected_relations, ranked),
            allowed_relations=tuple(sorted(profile.allowed_relations)),
            allowed_transitions=tuple(sorted(profile.allowed_transitions)),
            kind_bonus=dict(profile.kind_bonus),
            relation_weight_multiplier=dict(profile.relation_weight_multiplier),
            frontier_actions=dict(profile.frontier_actions),
        )
        for profile in _profiles_with_selected(selected_profile)
    )


def _is_compatible(
    profile: GraphProfile, selected_relations: set[str], ranked_nodes: tuple[Node, ...]
) -> bool:
    if not selected_relations.issubset(profile.allowed_relations):
        return False
    return all(frontier_action(profile, node) is not None for node in ranked_nodes)


def _profiles_with_selected(
    selected_profile: GraphProfile | None,
) -> tuple[GraphProfile, ...]:
    if selected_profile is None:
        return tuple(DEFAULT_PROFILES.values())
    return tuple(
        selected_profile if profile.name == selected_profile.name else profile
        for profile in DEFAULT_PROFILES.values()
    )
