"""Profile relation matrix summaries for graph-turbo results."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass

from .model import (
    GraphProfile,
    OrientedEdge,
    ProfileMatrixSummary,
    RelationChannelSummary,
    TypedGraph,
)
from .profiles import DEFAULT_PROFILES, allowed_oriented_edges


@dataclass(frozen=True, slots=True)
class _ProfileEdgeStats:
    selected_edges: tuple[OrientedEdge, ...]
    relation_edge_counts: Mapping[str, int]
    relation_weight_mass: Mapping[str, float]
    relation_matrix_nonzero_counts: Mapping[str, int]
    transition_nonzero_count: int
    transition_weight_mass: float


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
    edge_stats = _profile_edge_stats(graph, profile)
    relation_channels = _relation_channels(
        profile.allowed_relations,
        edge_stats,
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
        relation_matrix_count=len(profile.allowed_relations),
        zero_edge_relation_count=sum(
            1 for channel in relation_channels if channel.supported_edge_count == 0
        ),
        transition_nonzero_count=edge_stats.transition_nonzero_count,
        transition_weight_mass=round(edge_stats.transition_weight_mass, 6),
        relation_channels=relation_channels,
    )


def _relation_channels(
    allowed_relations: frozenset[str],
    edge_stats: _ProfileEdgeStats,
    reachable_node_ids: frozenset[str],
    ranked_node_ids: frozenset[str],
    frontier_node_ids: frozenset[str],
) -> tuple[RelationChannelSummary, ...]:
    reachable_counts: dict[str, int] = {}
    reachable_weight_mass: dict[str, float] = {}
    ranked_contribution_mass: dict[str, float] = {}
    frontier_contribution_mass: dict[str, float] = {}
    for edge in edge_stats.selected_edges:
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
            supported_edge_count=edge_stats.relation_edge_counts.get(relation, 0),
            reachable_edge_count=reachable_counts.get(relation, 0),
            weight_mass=round(edge_stats.relation_weight_mass.get(relation, 0.0), 6),
            reachable_weight_mass=round(reachable_weight_mass.get(relation, 0.0), 6),
            matrix_nonzero_count=edge_stats.relation_matrix_nonzero_counts.get(
                relation,
                0,
            ),
            ranked_contribution_mass=round(
                ranked_contribution_mass.get(relation, 0.0), 6
            ),
            frontier_contribution_mass=round(
                frontier_contribution_mass.get(relation, 0.0), 6
            ),
        )
        for relation in sorted(allowed_relations)
    )


def _profile_edge_stats(graph: TypedGraph, profile: GraphProfile) -> _ProfileEdgeStats:
    selected_edges: dict[tuple[str, str, str, str, str], OrientedEdge] = {}
    relation_edge_counts: dict[str, int] = {}
    relation_weight_mass: dict[str, float] = {}
    relation_pairs: dict[str, set[tuple[str, str]]] = {}
    transition_pairs: set[tuple[str, str]] = set()
    transition_sources: set[str] = set()
    for source_id, target_id, edge in allowed_oriented_edges(graph, profile):
        weight = edge.weight * profile.relation_weight_multiplier.get(
            edge.relation,
            1.0,
        )
        if weight <= 0.0:
            continue
        relation_edge_counts[edge.relation] = (
            relation_edge_counts.get(edge.relation, 0) + 1
        )
        relation_weight_mass[edge.relation] = (
            relation_weight_mass.get(edge.relation, 0.0) + weight
        )
        relation_pairs.setdefault(edge.relation, set()).add((source_id, target_id))
        transition_pairs.add((source_id, target_id))
        transition_sources.add(source_id)
        selected_edges[(source_id, target_id, edge.relation, edge.source, edge.target)] = (
            OrientedEdge(
                source=source_id,
                target=target_id,
                relation=edge.relation,
                original_source=edge.source,
                original_target=edge.target,
                reversed=source_id != edge.source or target_id != edge.target,
                weight=weight,
                fields=edge.fields,
            )
        )
    return _ProfileEdgeStats(
        selected_edges=tuple(selected_edges.values()),
        relation_edge_counts=relation_edge_counts,
        relation_weight_mass=relation_weight_mass,
        relation_matrix_nonzero_counts={
            relation: len(pairs) for relation, pairs in relation_pairs.items()
        },
        transition_nonzero_count=len(transition_pairs),
        transition_weight_mass=float(len(transition_sources)),
    )
