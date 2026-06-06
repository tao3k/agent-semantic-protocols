"""SciPy sparse backend for typed graph turbo ranking."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass

import numpy as np
from scipy.sparse import csr_matrix, diags
from scipy.sparse.csgraph import dijkstra

from .constants import DEFAULT_PAGERANK_ALPHA
from .model import Edge, GraphProfile, TypedGraph
from .profiles import allowed_oriented_edges


@dataclass(frozen=True)
class SparseGraphBackend:
    node_ids: tuple[str, ...]
    index_by_id: Mapping[str, int]
    adjacency: csr_matrix
    selected_edges: tuple[Edge, ...]


def build_sparse_backend(graph: TypedGraph, profile: GraphProfile) -> SparseGraphBackend:
    node_ids = tuple(graph.nodes)
    index_by_id = {node_id: index for index, node_id in enumerate(node_ids)}
    rows: list[int] = []
    cols: list[int] = []
    weights: list[float] = []
    selected_edges: dict[tuple[str, str, str], Edge] = {}
    for source_id, target_id, edge in allowed_oriented_edges(graph, profile):
        source = index_by_id[source_id]
        target = index_by_id[target_id]
        rows.append(source)
        cols.append(target)
        weights.append(edge.weight)
        selected_edges[(edge.source, edge.target, edge.relation)] = edge
    adjacency = csr_matrix((weights, (rows, cols)), shape=(len(node_ids), len(node_ids)))
    return SparseGraphBackend(node_ids, index_by_id, adjacency, tuple(selected_edges.values()))


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


def typed_personalized_pagerank(
    backend: SparseGraphBackend,
    seed_ids: Iterable[str],
    *,
    alpha: float = DEFAULT_PAGERANK_ALPHA,
    max_iter: int = 100,
    tolerance: float = 1e-9,
) -> dict[str, float]:
    node_count = len(backend.node_ids)
    if node_count == 0:
        return {}
    personalization = _personalization_vector(backend, seed_ids)
    if node_count == 1:
        return {backend.node_ids[0]: 1.0}
    transition = _row_stochastic_transition(backend.adjacency)
    dangling = np.asarray(backend.adjacency.sum(axis=1)).ravel() == 0.0
    rank = personalization.copy()
    for _ in range(max_iter):
        dangling_mass = float(rank[dangling].sum()) if dangling.any() else 0.0
        next_rank = alpha * ((transition.T @ rank) + (dangling_mass * personalization))
        next_rank += (1.0 - alpha) * personalization
        if np.abs(next_rank - rank).sum() < tolerance:
            rank = next_rank
            break
        rank = next_rank
    return {
        node_id: float(rank[index])
        for index, node_id in enumerate(backend.node_ids)
    }


def reachable_edges(
    backend: SparseGraphBackend, best_depth: Mapping[str, int]
) -> dict[tuple[str, str, str], Edge]:
    return {
        (edge.source, edge.target, edge.relation): edge
        for edge in backend.selected_edges
        if edge.source in best_depth and edge.target in best_depth
    }


def _seed_indexes(backend: SparseGraphBackend, seed_ids: Iterable[str]) -> tuple[int, ...]:
    return tuple(
        backend.index_by_id[node_id] for node_id in seed_ids if node_id in backend.index_by_id
    )


def _personalization_vector(
    backend: SparseGraphBackend, seed_ids: Iterable[str]
) -> np.ndarray:
    node_count = len(backend.node_ids)
    vector = np.zeros(node_count, dtype=float)
    seed_indexes = _seed_indexes(backend, seed_ids)
    if seed_indexes:
        seed_weight = 1.0 / len(seed_indexes)
        for index in seed_indexes:
            vector[index] = seed_weight
    else:
        vector.fill(1.0 / node_count)
    return vector


def _row_stochastic_transition(adjacency: csr_matrix) -> csr_matrix:
    row_sums = np.asarray(adjacency.sum(axis=1)).ravel()
    inverse = np.zeros_like(row_sums, dtype=float)
    np.divide(1.0, row_sums, out=inverse, where=row_sums > 0.0)
    return diags(inverse).dot(adjacency).tocsr()
