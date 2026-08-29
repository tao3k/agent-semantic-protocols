"""Shared SciPy graph backend for graph turbo typed paths."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

import numpy as np
from scipy.sparse import csr_matrix

from .model import Edge, GraphProfile, TypedGraph
from .profiles import allowed_oriented_edges

GraphTurboPathCandidate = tuple[str, str, tuple[str, ...], tuple[str, ...], float]


@dataclass(frozen=True)
class ScipyPathBackend:
    node_ids: tuple[str, ...]
    index_by_id: Mapping[str, int]
    cost_matrix: csr_matrix
    relation_by_pair: Mapping[tuple[str, str], str]


def build_scipy_path_backend(
    graph: TypedGraph, profile: GraphProfile
) -> ScipyPathBackend:
    node_ids = tuple(graph.nodes)
    index_by_id = {node_id: index for index, node_id in enumerate(node_ids)}
    best_edge_by_pair: dict[tuple[str, str], Edge] = {}
    for source, target, edge in allowed_oriented_edges(graph, profile):
        pair = (source, target)
        current = best_edge_by_pair.get(pair)
        if current is None or edge_cost(edge) < edge_cost(current):
            best_edge_by_pair[pair] = edge
    rows = [index_by_id[source] for source, _target in best_edge_by_pair]
    cols = [index_by_id[target] for _source, target in best_edge_by_pair]
    costs = [edge_cost(edge) for edge in best_edge_by_pair.values()]
    return ScipyPathBackend(
        node_ids=node_ids,
        index_by_id=index_by_id,
        cost_matrix=csr_matrix(
            (costs, (rows, cols)), shape=(len(node_ids), len(node_ids))
        ),
        relation_by_pair={
            pair: edge.relation for pair, edge in best_edge_by_pair.items()
        },
    )


def reconstruct_node_path(
    node_ids: tuple[str, ...],
    predecessors: np.ndarray,
    *,
    source_index: int,
    sink_index: int,
) -> tuple[str, ...]:
    reversed_indexes = [sink_index]
    current = sink_index
    while current != source_index:
        predecessor = int(predecessors[current])
        if predecessor < 0 or predecessor == current:
            return ()
        reversed_indexes.append(predecessor)
        current = predecessor
    return tuple(node_ids[index] for index in reversed(reversed_indexes))


def relations_for_path(
    node_ids: tuple[str, ...], relation_by_pair: Mapping[tuple[str, str], str]
) -> tuple[str, ...]:
    relations = tuple(
        relation_by_pair.get((source, target), "")
        for source, target in zip(node_ids, node_ids[1:], strict=False)
    )
    return () if any(not relation for relation in relations) else relations


def edge_cost(edge: Edge) -> float:
    return 1.0 / edge.weight if edge.weight > 0.0 else 999.0
