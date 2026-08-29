"""SciPy Yen k-shortest typed-path adapter for graph turbo."""

from __future__ import annotations

from collections.abc import Iterator

import numpy as np
from scipy.sparse.csgraph import yen

from .model import GraphProfile, SourceSinkFrontier, TypedGraph
from .path_scipy_backend import (
    GraphTurboPathCandidate,
    ScipyPathBackend,
    build_scipy_path_backend,
    reconstruct_node_path,
    relations_for_path,
)


def graph_turbo_scipy_yen_path_candidates(
    graph: TypedGraph,
    profile: GraphProfile,
    frontier: SourceSinkFrontier,
    *,
    max_hops: int,
    path_budget: int,
) -> tuple[GraphTurboPathCandidate, ...]:
    backend = build_scipy_path_backend(graph, profile)
    return tuple(
        candidate
        for source in frontier.source_ids
        for sink in frontier.sink_ids
        for candidate in _yen_pair_candidates(
            backend,
            source,
            sink,
            max_hops=max_hops,
            path_budget=path_budget,
        )
    )


def _yen_pair_candidates(
    backend: ScipyPathBackend,
    source: str,
    sink: str,
    *,
    max_hops: int,
    path_budget: int,
) -> tuple[GraphTurboPathCandidate, ...]:
    source_index = backend.index_by_id.get(source)
    sink_index = backend.index_by_id.get(sink)
    if source_index is None or sink_index is None:
        return ()
    distances, predecessors = yen(
        backend.cost_matrix,
        source_index,
        sink_index,
        max(path_budget, 1),
        directed=True,
        return_predecessors=True,
    )
    return tuple(
        candidate
        for distance, predecessor_row in _yen_result_rows(distances, predecessors)
        if (
            candidate := _predecessor_path_candidate(
                backend,
                source,
                sink,
                source_index=source_index,
                sink_index=sink_index,
                distance=float(distance),
                predecessors=predecessor_row,
                max_hops=max_hops,
            )
        )
        is not None
    )


def _yen_result_rows(
    distances: np.ndarray, predecessors: np.ndarray
) -> Iterator[tuple[float, np.ndarray]]:
    distance_rows = np.atleast_1d(distances)
    predecessor_rows = np.atleast_2d(predecessors)
    return zip(distance_rows, predecessor_rows, strict=False)


def _predecessor_path_candidate(
    backend: ScipyPathBackend,
    source: str,
    sink: str,
    *,
    source_index: int,
    sink_index: int,
    distance: float,
    predecessors: np.ndarray,
    max_hops: int,
) -> GraphTurboPathCandidate | None:
    if not np.isfinite(distance):
        return None
    node_ids = reconstruct_node_path(
        backend.node_ids,
        predecessors,
        source_index=source_index,
        sink_index=sink_index,
    )
    if not node_ids or len(node_ids) - 1 > max_hops:
        return None
    relations = relations_for_path(node_ids, backend.relation_by_pair)
    if len(relations) != len(node_ids) - 1:
        return None
    return (source, sink, node_ids, relations, distance)
