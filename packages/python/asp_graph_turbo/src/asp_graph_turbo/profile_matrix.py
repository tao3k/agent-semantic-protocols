"""Profile relation matrix summaries for graph-turbo results."""

from __future__ import annotations

from collections.abc import Iterable

from .backend import SparseGraphBackend, build_sparse_backend
from .model import (
    GraphProfile,
    ProfileMatrixSummary,
    RelationChannelSummary,
    TypedGraph,
)
from .profiles import DEFAULT_PROFILES


def profile_matrix_bank(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    reachable_node_ids: frozenset[str],
    ranked_node_ids: Iterable[str] = (),
    frontier_node_ids: Iterable[str] = (),
) -> tuple[ProfileMatrixSummary, ...]:
    ranked_ids = frozenset(ranked_node_ids)
    frontier_ids = frozenset(frontier_node_ids)
    return tuple(
        _profile_matrix_summary(
            graph,
            profile,
            reachable_node_ids
            if profile.name == selected_profile.name
            else frozenset(),
            ranked_ids if profile.name == selected_profile.name else frozenset(),
            frontier_ids if profile.name == selected_profile.name else frozenset(),
        )
        for profile in _profiles_with_selected(selected_profile)
    )


def _profiles_with_selected(selected_profile: GraphProfile) -> tuple[GraphProfile, ...]:
    return tuple(
        selected_profile if profile.name == selected_profile.name else profile
        for profile in DEFAULT_PROFILES.values()
    )


def _profile_matrix_summary(
    graph: TypedGraph,
    profile: GraphProfile,
    reachable_node_ids: frozenset[str],
    ranked_node_ids: frozenset[str],
    frontier_node_ids: frozenset[str],
) -> ProfileMatrixSummary:
    backend = build_sparse_backend(graph, profile)
    relation_channels = _relation_channels(
        profile.allowed_relations,
        backend,
        reachable_node_ids,
        ranked_node_ids,
        frontier_node_ids,
    )
    reachable_edge_count = sum(
        channel.reachable_edge_count for channel in relation_channels
    )
    supported_edge_count = sum(
        channel.supported_edge_count for channel in relation_channels
    )
    node_count = max(len(graph.nodes), 1)
    density = supported_edge_count / float(node_count * node_count)
    return ProfileMatrixSummary(
        profile=profile.name,
        relation_count=len(profile.allowed_relations),
        transition_count=len(profile.allowed_transitions),
        supported_edge_count=supported_edge_count,
        reachable_edge_count=reachable_edge_count,
        density=round(density, 6),
        relation_matrix_count=len(backend.relation_matrices),
        zero_edge_relation_count=sum(
            1 for channel in relation_channels if channel.supported_edge_count == 0
        ),
        transition_nonzero_count=int(backend.transition.nnz),
        transition_weight_mass=round(float(backend.transition.sum()), 6),
        relation_channels=relation_channels,
    )


def _relation_channels(
    allowed_relations: frozenset[str],
    backend: SparseGraphBackend,
    reachable_node_ids: frozenset[str],
    ranked_node_ids: frozenset[str],
    frontier_node_ids: frozenset[str],
) -> tuple[RelationChannelSummary, ...]:
    reachable_counts: dict[str, int] = {}
    reachable_weight_mass: dict[str, float] = {}
    ranked_contribution_mass: dict[str, float] = {}
    frontier_contribution_mass: dict[str, float] = {}
    for edge in backend.selected_edges:
        relation = edge.relation
        if edge.source in reachable_node_ids and edge.target in reachable_node_ids:
            reachable_counts[relation] = reachable_counts.get(relation, 0) + 1
            reachable_weight_mass[relation] = (
                reachable_weight_mass.get(relation, 0.0) + edge.weight
            )
        if edge.target in ranked_node_ids:
            ranked_contribution_mass[relation] = (
                ranked_contribution_mass.get(relation, 0.0) + edge.weight
            )
        if edge.target in frontier_node_ids:
            frontier_contribution_mass[relation] = (
                frontier_contribution_mass.get(relation, 0.0) + edge.weight
            )
    return tuple(
        RelationChannelSummary(
            relation=relation,
            supported_edge_count=backend.relation_edge_counts.get(relation, 0),
            reachable_edge_count=reachable_counts.get(relation, 0),
            weight_mass=round(backend.relation_weight_mass.get(relation, 0.0), 6),
            reachable_weight_mass=round(reachable_weight_mass.get(relation, 0.0), 6),
            matrix_nonzero_count=int(backend.relation_matrices[relation].nnz),
            ranked_contribution_mass=round(
                ranked_contribution_mass.get(relation, 0.0), 6
            ),
            frontier_contribution_mass=round(
                frontier_contribution_mass.get(relation, 0.0), 6
            ),
        )
        for relation in sorted(allowed_relations)
    )
