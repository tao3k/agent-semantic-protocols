"""SciPy Dijkstra typed-path adapter for graph turbo."""

from __future__ import annotations

import numpy as np
from scipy.sparse.csgraph import dijkstra

from .model import GraphProfile, SourceSinkFrontier, TypedGraph
from .path_scipy_backend import (
    GraphTurboPathCandidate,
    ScipyPathBackend,
    build_scipy_path_backend,
    reconstruct_node_path,
    relations_for_path,
)


def graph_turbo_scipy_path_candidates(
    graph: TypedGraph,
    profile: GraphProfile,
    frontier: SourceSinkFrontier,
    *,
    max_hops: int,
) -> tuple[GraphTurboPathCandidate, ...]:
    backend = build_scipy_path_backend(graph, profile)
    return tuple(
        candidate
        for source in frontier.source_ids
        for candidate in _source_path_candidates(
            backend,
            source,
            frontier.sink_ids,
            max_hops=max_hops,
        )
    )


def _source_path_candidates(
    backend: ScipyPathBackend,
    source: str,
    sink_ids: tuple[str, ...],
    *,
    max_hops: int,
) -> tuple[GraphTurboPathCandidate, ...]:
    source_index = backend.index_by_id.get(source)
    if source_index is None:
        return ()
    distances, predecessors = dijkstra(
        backend.cost_matrix,
        directed=True,
        indices=source_index,
        return_predecessors=True,
    )
    return tuple(
        candidate
        for sink in sink_ids
        if (
            candidate := _sink_path_candidate(
                backend,
                source,
                sink,
                source_index=source_index,
                distances=distances,
                predecessors=predecessors,
                max_hops=max_hops,
            )
        )
        is not None
    )


def _sink_path_candidate(
    backend: ScipyPathBackend,
    source: str,
    sink: str,
    *,
    source_index: int,
    distances: np.ndarray,
    predecessors: np.ndarray,
    max_hops: int,
) -> GraphTurboPathCandidate | None:
    sink_index = backend.index_by_id.get(sink)
    if sink_index is None or not np.isfinite(distances[sink_index]):
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
    return (source, sink, node_ids, relations, float(distances[sink_index]))
