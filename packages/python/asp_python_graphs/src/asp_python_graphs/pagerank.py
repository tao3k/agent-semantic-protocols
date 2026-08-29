"""Typed personalized PageRank diagnostics for graph turbo."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from typing import TYPE_CHECKING

import numpy as np

from .constants import DEFAULT_PAGERANK_ALPHA

if TYPE_CHECKING:
    from .backend import SparseGraphBackend


@dataclass(frozen=True)
class GraphTurboPprResult:
    scores: Mapping[str, float]
    iterations: int
    residual: float
    dangling_mass_last: float
    mass_sum: float


def graph_turbo_typed_personalized_pagerank(
    backend: SparseGraphBackend,
    seed_ids: Iterable[str],
    *,
    seed_weights: Mapping[str, float] | None = None,
    alpha: float = DEFAULT_PAGERANK_ALPHA,
    max_iter: int = 100,
    tolerance: float = 1e-9,
) -> dict[str, float]:
    return dict(
        graph_turbo_typed_personalized_pagerank_result(
            backend,
            seed_ids,
            seed_weights=seed_weights,
            alpha=alpha,
            max_iter=max_iter,
            tolerance=tolerance,
        ).scores
    )


def graph_turbo_typed_personalized_pagerank_result(
    backend: SparseGraphBackend,
    seed_ids: Iterable[str],
    *,
    seed_weights: Mapping[str, float] | None = None,
    alpha: float = DEFAULT_PAGERANK_ALPHA,
    max_iter: int = 100,
    tolerance: float = 1e-9,
) -> GraphTurboPprResult:
    node_count = len(backend.node_ids)
    if node_count == 0:
        return GraphTurboPprResult({}, 0, 0.0, 0.0, 0.0)
    personalization = _personalization_vector(
        backend,
        seed_ids,
        seed_weights=seed_weights,
    )
    if node_count == 1:
        return GraphTurboPprResult({backend.node_ids[0]: 1.0}, 0, 0.0, 0.0, 1.0)
    dangling = np.asarray(backend.adjacency.sum(axis=1)).ravel() == 0.0
    rank = personalization.copy()
    residual = 0.0
    dangling_mass = 0.0
    iterations = 0
    for iterations in range(1, max_iter + 1):
        dangling_mass = float(rank[dangling].sum()) if dangling.any() else 0.0
        next_rank = alpha * (
            (backend.transition.T @ rank) + (dangling_mass * personalization)
        )
        next_rank += (1.0 - alpha) * personalization
        residual = float(np.abs(next_rank - rank).sum())
        rank = next_rank
        if residual < tolerance:
            break
    return GraphTurboPprResult(
        scores={
            node_id: float(rank[index])
            for index, node_id in enumerate(backend.node_ids)
        },
        iterations=iterations,
        residual=residual,
        dangling_mass_last=dangling_mass,
        mass_sum=float(rank.sum()),
    )


def _seed_indexes(
    backend: SparseGraphBackend, seed_ids: Iterable[str]
) -> tuple[tuple[str, int], ...]:
    return tuple(
        (node_id, backend.index_by_id[node_id])
        for node_id in seed_ids
        if node_id in backend.index_by_id
    )


def _personalization_vector(
    backend: SparseGraphBackend,
    seed_ids: Iterable[str],
    *,
    seed_weights: Mapping[str, float] | None = None,
) -> np.ndarray:
    node_count = len(backend.node_ids)
    vector = np.zeros(node_count, dtype=float)
    seed_entries = _seed_indexes(backend, seed_ids)
    if seed_entries:
        weighted_entries = tuple(
            (index, _seed_weight(node_id, seed_weights))
            for node_id, index in seed_entries
        )
        total_weight = sum(weight for _, weight in weighted_entries)
        if total_weight <= 0.0:
            seed_weight = 1.0 / len(weighted_entries)
            for index, _ in weighted_entries:
                vector[index] = seed_weight
        else:
            for index, weight in weighted_entries:
                vector[index] = weight / total_weight
    else:
        vector.fill(1.0 / node_count)
    return vector


def _seed_weight(
    node_id: str,
    seed_weights: Mapping[str, float] | None,
) -> float:
    if seed_weights is None:
        return 1.0
    weight = seed_weights.get(node_id, 1.0)
    return weight if weight > 0.0 else 0.0
