"""Sparse-matrix execution for encoded SearchLoop routes."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping

import numpy as np
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import dijkstra

from .graph_model import TypedGraph
from .path_scipy_searchloop_model import EncodedEdge


@dataclass(frozen=True, slots=True)
class ScipyPathProjection:
    node_ids: tuple[str, ...]
    source_index: int
    sink_index: int
    distance: float
    predecessors: np.ndarray

    def path_indices(self) -> tuple[int, ...]:
        reversed_indices = [self.sink_index]
        cursor = self.sink_index
        while cursor != self.source_index:
            predecessor = int(self.predecessors[cursor])
            if predecessor < 0:
                return ()
            reversed_indices.append(predecessor)
            cursor = predecessor
        return tuple(reversed(reversed_indices))


def best_edges_by_pair(
    encoded_edges: tuple[EncodedEdge, ...],
) -> dict[tuple[str, str], EncodedEdge]:
    best: dict[tuple[str, str], EncodedEdge] = {}
    for candidate in encoded_edges:
        pair = (candidate.edge.source, candidate.edge.target)
        incumbent = best.get(pair)
        candidate_key = (candidate.encoded_cost, candidate.edge.relation)
        if incumbent is None or candidate_key < (
            incumbent.encoded_cost,
            incumbent.edge.relation,
        ):
            best[pair] = candidate
    return best


def _cost_matrix(
    node_ids: tuple[str, ...],
    index_by_id: Mapping[str, int],
    pair_edges: Mapping[tuple[str, str], EncodedEdge],
) -> csr_matrix:
    rows: list[int] = []
    columns: list[int] = []
    values: list[float] = []
    for (source, target), edge in sorted(pair_edges.items()):
        if source not in index_by_id or target not in index_by_id:
            raise ValueError("edge endpoint is absent from graph.nodes")
        rows.append(index_by_id[source])
        columns.append(index_by_id[target])
        values.append(float(edge.encoded_cost))
    return csr_matrix(
        (np.asarray(values), (np.asarray(rows), np.asarray(columns))),
        shape=(len(node_ids), len(node_ids)),
        dtype=np.float64,
    )


def scipy_shortest_path(
    graph: TypedGraph,
    pair_edges: Mapping[tuple[str, str], EncodedEdge],
    *,
    source: str,
    sink: str,
) -> ScipyPathProjection:
    node_ids = tuple(sorted(graph.nodes))
    index_by_id = {node_id: index for index, node_id in enumerate(node_ids)}
    source_index = index_by_id[source]
    sink_index = index_by_id[sink]
    distances, predecessors = dijkstra(
        _cost_matrix(node_ids, index_by_id, pair_edges),
        directed=True,
        indices=source_index,
        return_predecessors=True,
    )
    return ScipyPathProjection(
        node_ids=node_ids,
        source_index=source_index,
        sink_index=sink_index,
        distance=float(distances[sink_index]),
        predecessors=predecessors,
    )
