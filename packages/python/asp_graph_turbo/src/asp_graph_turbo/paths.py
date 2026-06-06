"""Typed path and flow-lite evidence for graph turbo."""

from __future__ import annotations

from collections import deque
from collections.abc import Mapping

from .model import Edge, FlowLite, GraphProfile, SourceSinkFrontier, TypedGraph, TypedPath
from .profiles import allowed_oriented_edges, frontier_action


def source_sink_frontier(
    graph: TypedGraph,
    profile: GraphProfile,
    seed_ids: tuple[str, ...],
    ranked_node_ids: tuple[str, ...],
) -> SourceSinkFrontier:
    sinks = tuple(
        node_id
        for node_id in ranked_node_ids
        if node_id not in seed_ids and frontier_action(profile, graph.nodes[node_id]) is not None
    )
    return SourceSinkFrontier(seed_ids, sinks)


def typed_paths(
    graph: TypedGraph,
    profile: GraphProfile,
    frontier: SourceSinkFrontier,
    scores: Mapping[str, float],
    *,
    path_budget: int,
    max_hops: int,
) -> tuple[TypedPath, ...]:
    adjacency = _adjacency(graph, profile)
    candidates: list[tuple[str, str, tuple[str, ...], tuple[str, ...], float]] = []
    for source in frontier.source_ids:
        for sink in frontier.sink_ids:
            candidates.extend(_simple_paths(adjacency, source, sink, max_hops))
    unique_candidates = _dedupe_paths(candidates)
    unique_candidates.sort(key=lambda item: (item[4], len(item[2]), item[0], item[1], item[2]))
    typed: list[TypedPath] = []
    for index, candidate in enumerate(unique_candidates[:path_budget], start=1):
        source, sink, node_ids, relations, cost = candidate
        path_kind = "constrained-shortest" if index == 1 else "k-shortest"
        typed.append(
            TypedPath(
                id=f"P{index}",
                path_kind=path_kind,
                source=source,
                sink=sink,
                node_ids=node_ids,
                relations=relations,
                cost=cost,
                score=_path_score(node_ids, scores),
                rank=0,
            )
        )
    typed.sort(key=lambda item: (-item.score, item.cost, item.id))
    return tuple(
        TypedPath(
            id=item.id,
            path_kind=item.path_kind,
            source=item.source,
            sink=item.sink,
            node_ids=item.node_ids,
            relations=item.relations,
            cost=item.cost,
            score=item.score,
            rank=rank,
        )
        for rank, item in enumerate(typed, start=1)
    )


def flow_lite(paths: tuple[TypedPath, ...]) -> FlowLite:
    return FlowLite(tuple(path.id for path in sorted(paths, key=lambda item: item.rank)))


def _adjacency(
    graph: TypedGraph, profile: GraphProfile
) -> dict[str, list[tuple[str, Edge]]]:
    adjacency: dict[str, list[tuple[str, Edge]]] = {node_id: [] for node_id in graph.nodes}
    for source, target, edge in allowed_oriented_edges(graph, profile):
        adjacency[source].append((target, edge))
    for neighbors in adjacency.values():
        neighbors.sort(key=lambda item: (item[0], item[1].relation))
    return adjacency


def _simple_paths(
    adjacency: Mapping[str, list[tuple[str, Edge]]],
    source: str,
    sink: str,
    max_hops: int,
) -> list[tuple[str, str, tuple[str, ...], tuple[str, ...]]]:
    if source not in adjacency or sink not in adjacency:
        return []
    queue: deque[tuple[str, tuple[str, ...], tuple[str, ...], float]] = deque(
        [(source, (source,), (), 0.0)]
    )
    paths: list[tuple[str, str, tuple[str, ...], tuple[str, ...], float]] = []
    while queue:
        node_id, node_path, relations, cost = queue.popleft()
        if len(node_path) - 1 > max_hops:
            continue
        if node_id == sink:
            paths.append((source, sink, node_path, relations, cost))
            continue
        for neighbor, edge in adjacency[node_id]:
            if neighbor in node_path:
                continue
            queue.append(
                (
                    neighbor,
                    (*node_path, neighbor),
                    (*relations, edge.relation),
                    cost + _edge_cost(edge),
                )
            )
    return paths


def _dedupe_paths(
    candidates: list[tuple[str, str, tuple[str, ...], tuple[str, ...], float]]
) -> list[tuple[str, str, tuple[str, ...], tuple[str, ...], float]]:
    seen: set[tuple[str, ...]] = set()
    unique: list[tuple[str, str, tuple[str, ...], tuple[str, ...], float]] = []
    for candidate in candidates:
        key = candidate[2]
        if key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def _path_score(node_ids: tuple[str, ...], scores: Mapping[str, float]) -> float:
    if not node_ids:
        return 0.0
    total = sum(scores.get(node_id, 0.0) for node_id in node_ids)
    return total / len(node_ids)


def _edge_cost(edge: Edge) -> float:
    return 1.0 / edge.weight if edge.weight > 0.0 else 999.0
