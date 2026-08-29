"""SciPy sparse backend for typed graph turbo ranking."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass

import numpy as np
from scipy.sparse import csr_matrix, diags
from scipy.sparse.csgraph import dijkstra

from .model import Edge, GraphProfile, OrientedEdge, TypedGraph
from .profiles import allowed_oriented_edges


@dataclass(frozen=True)
class SparseGraphBackend:
    node_ids: tuple[str, ...]
    index_by_id: Mapping[str, int]
    adjacency: csr_matrix
    transition: csr_matrix
    relation_matrices: Mapping[str, csr_matrix]
    relation_edge_counts: Mapping[str, int]
    relation_weight_mass: Mapping[str, float]
    selected_edges: tuple[OrientedEdge, ...]


def build_sparse_backend(
    graph: TypedGraph, profile: GraphProfile
) -> SparseGraphBackend:
    node_ids = tuple(graph.nodes)
    index_by_id = {node_id: index for index, node_id in enumerate(node_ids)}
    rows: list[int] = []
    cols: list[int] = []
    weights: list[float] = []
    relation_rows: dict[str, list[int]] = {}
    relation_cols: dict[str, list[int]] = {}
    relation_weights: dict[str, list[float]] = {}
    relation_weight_mass: dict[str, float] = {}
    selected_edges: dict[tuple[str, str, str, str, str], OrientedEdge] = {}
    for source_id, target_id, edge in allowed_oriented_edges(graph, profile):
        edge_weight = _profile_edge_weight(edge, profile)
        if edge_weight <= 0.0:
            continue
        source = index_by_id[source_id]
        target = index_by_id[target_id]
        oriented_edge = _oriented_edge(source_id, target_id, edge, edge_weight)
        rows.append(source)
        cols.append(target)
        weights.append(edge_weight)
        relation_rows.setdefault(edge.relation, []).append(source)
        relation_cols.setdefault(edge.relation, []).append(target)
        relation_weights.setdefault(edge.relation, []).append(edge_weight)
        relation_weight_mass[edge.relation] = (
            relation_weight_mass.get(edge.relation, 0.0) + edge_weight
        )
        selected_edges[
            (source_id, target_id, edge.relation, edge.source, edge.target)
        ] = oriented_edge
    adjacency = csr_matrix(
        (weights, (rows, cols)), shape=(len(node_ids), len(node_ids))
    )
    return SparseGraphBackend(
        node_ids=node_ids,
        index_by_id=index_by_id,
        adjacency=adjacency,
        transition=_row_stochastic_transition(adjacency),
        relation_matrices=_relation_matrices(
            relation_rows,
            relation_cols,
            relation_weights,
            node_count=len(node_ids),
            relations=profile.allowed_relations,
        ),
        relation_edge_counts={
            relation: len(relation_weights.get(relation, ()))
            for relation in sorted(profile.allowed_relations)
        },
        relation_weight_mass={
            relation: relation_weight_mass.get(relation, 0.0)
            for relation in sorted(profile.allowed_relations)
        },
        selected_edges=tuple(selected_edges.values()),
    )


def sparse_backend_from_parts(
    node_ids: tuple[str, ...],
    adjacency: csr_matrix,
    selected_edges: tuple[OrientedEdge, ...],
    *,
    relations: Iterable[str] = (),
) -> SparseGraphBackend:
    index_by_id = {node_id: index for index, node_id in enumerate(node_ids)}
    relation_rows: dict[str, list[int]] = {}
    relation_cols: dict[str, list[int]] = {}
    relation_weights: dict[str, list[float]] = {}
    relation_weight_mass: dict[str, float] = {}
    for edge in selected_edges:
        if edge.source not in index_by_id or edge.target not in index_by_id:
            continue
        relation_rows.setdefault(edge.relation, []).append(index_by_id[edge.source])
        relation_cols.setdefault(edge.relation, []).append(index_by_id[edge.target])
        relation_weights.setdefault(edge.relation, []).append(edge.weight)
        relation_weight_mass[edge.relation] = (
            relation_weight_mass.get(edge.relation, 0.0) + edge.weight
        )
    return SparseGraphBackend(
        node_ids=node_ids,
        index_by_id=index_by_id,
        adjacency=adjacency,
        transition=_row_stochastic_transition(adjacency),
        relation_matrices=_relation_matrices(
            relation_rows,
            relation_cols,
            relation_weights,
            node_count=len(node_ids),
            relations=frozenset(relations) | frozenset(relation_weights),
        ),
        relation_edge_counts={
            relation: len(relation_weights.get(relation, ()))
            for relation in sorted(frozenset(relations) | frozenset(relation_weights))
        },
        relation_weight_mass={
            relation: relation_weight_mass.get(relation, 0.0)
            for relation in sorted(frozenset(relations) | frozenset(relation_weights))
        },
        selected_edges=selected_edges,
    )


def multi_source_hop_lengths(
    backend: SparseGraphBackend, seed_ids: Iterable[str], max_depth: int
) -> dict[str, int]:
    seed_indexes = _seed_indexes(backend, seed_ids)
    if not seed_indexes:
        return {}
    distances = dijkstra(
        backend.adjacency,
        directed=True,
        indices=seed_indexes,
        unweighted=True,
        limit=max_depth,
    )
    matrix = np.atleast_2d(distances)
    min_distances = matrix.min(axis=0)
    return {
        backend.node_ids[index]: int(distance)
        for index, distance in enumerate(min_distances)
        if np.isfinite(distance) and distance <= max_depth
    }


def reachable_edges(
    backend: SparseGraphBackend, best_depth: Mapping[str, int]
) -> dict[tuple[str, str, str], OrientedEdge]:
    return {
        (edge.source, edge.target, edge.relation): edge
        for edge in backend.selected_edges
        if edge.source in best_depth and edge.target in best_depth
    }


def _seed_indexes(
    backend: SparseGraphBackend, seed_ids: Iterable[str]
) -> tuple[int, ...]:
    return tuple(
        backend.index_by_id[node_id]
        for node_id in seed_ids
        if node_id in backend.index_by_id
    )


def _row_stochastic_transition(adjacency: csr_matrix) -> csr_matrix:
    row_sums = np.asarray(adjacency.sum(axis=1)).ravel()
    inverse = np.zeros_like(row_sums, dtype=float)
    np.divide(1.0, row_sums, out=inverse, where=row_sums > 0.0)
    return diags(inverse).dot(adjacency).tocsr()


def _relation_matrices(
    relation_rows: Mapping[str, list[int]],
    relation_cols: Mapping[str, list[int]],
    relation_weights: Mapping[str, list[float]],
    *,
    node_count: int,
    relations: Iterable[str],
) -> Mapping[str, csr_matrix]:
    return {
        relation: csr_matrix(
            (
                relation_weights.get(relation, ()),
                (
                    relation_rows.get(relation, ()),
                    relation_cols.get(relation, ()),
                ),
            ),
            shape=(node_count, node_count),
        )
        for relation in sorted(relations)
    }


def _profile_edge_weight(edge: Edge, profile: GraphProfile) -> float:
    return edge.weight * profile.relation_weight_multiplier.get(edge.relation, 1.0)


def _oriented_edge(
    source_id: str,
    target_id: str,
    edge: Edge,
    weight: float,
) -> OrientedEdge:
    return OrientedEdge(
        source=source_id,
        target=target_id,
        relation=edge.relation,
        original_source=edge.source,
        original_target=edge.target,
        reversed=source_id != edge.source or target_id != edge.target,
        weight=weight,
        fields=edge.fields,
    )
