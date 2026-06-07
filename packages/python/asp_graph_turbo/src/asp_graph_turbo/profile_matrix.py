"""Profile relation matrix summaries for graph-turbo results."""

from __future__ import annotations

from .model import GraphProfile, ProfileMatrixSummary, TypedGraph
from .profiles import DEFAULT_PROFILES, allowed_oriented_edges


def profile_matrix_bank(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    reachable_node_ids: frozenset[str],
) -> tuple[ProfileMatrixSummary, ...]:
    return tuple(
        _profile_matrix_summary(
            graph,
            profile,
            reachable_node_ids if profile.name == selected_profile.name else frozenset(),
        )
        for profile in DEFAULT_PROFILES.values()
    )


def _profile_matrix_summary(
    graph: TypedGraph, profile: GraphProfile, reachable_node_ids: frozenset[str]
) -> ProfileMatrixSummary:
    oriented_edges = allowed_oriented_edges(graph, profile)
    reachable_edge_count = sum(
        1
        for source, target, _edge in oriented_edges
        if source in reachable_node_ids and target in reachable_node_ids
    )
    node_count = max(len(graph.nodes), 1)
    density = len(oriented_edges) / float(node_count * node_count)
    return ProfileMatrixSummary(
        profile=profile.name,
        relation_count=len(profile.allowed_relations),
        transition_count=len(profile.allowed_transitions),
        supported_edge_count=len(oriented_edges),
        reachable_edge_count=reachable_edge_count,
        density=round(density, 6),
    )
