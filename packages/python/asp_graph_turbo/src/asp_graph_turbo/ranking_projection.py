"""Graph-turbo ranked/path projection artifacts."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass

from .compatibility import profile_compatibility
from .model import (
    FlowLite,
    FrontierEntry,
    GraphProfile,
    MergedWindow,
    Node,
    OrientedEdge,
    ProfileCompatibility,
    ProfileMatrixSummary,
    ReadLoopGuard,
    SourceSinkFrontier,
    TypedGraph,
    TypedPath,
)
from .paths import flow_lite, source_sink_frontier, typed_paths_with_backend
from .profile_matrix import profile_matrix_bank
from .profiles import frontier_action
from .read_loop_guard import evaluate_read_loop_guard
from .windows import merge_ranked_windows


@dataclass(frozen=True, slots=True)
class RankedProjection:
    frontier: tuple[FrontierEntry, ...]
    edges: tuple[OrientedEdge, ...]
    merged_windows: tuple[MergedWindow, ...]
    read_loop_guard: ReadLoopGuard
    compatibility: tuple[ProfileCompatibility, ...]
    matrices: tuple[ProfileMatrixSummary, ...]
    ranked_ids: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class PathProjection:
    source_sink: SourceSinkFrontier
    paths: tuple[TypedPath, ...]
    flow: FlowLite
    backend: str
    fallback_count: int
    pair_count: int
    candidate_count: int


def build_ranked_projection(
    *,
    graph: TypedGraph,
    selected_profile: GraphProfile,
    scores: Mapping[str, float],
    selected_edges: Iterable[OrientedEdge],
    best_depth: Mapping[str, int],
    ranked: tuple[Node, ...],
    window_merge_enabled: bool,
    window_merge_max_gap_lines: int,
) -> RankedProjection:
    frontier = _frontier_entries(selected_profile, ranked, scores)
    edges = _ranked_edges(selected_edges, ranked)
    merged_windows = merge_ranked_windows(
        ranked,
        enabled=window_merge_enabled,
        max_gap_lines=window_merge_max_gap_lines,
    )
    read_loop_guard = evaluate_read_loop_guard(
        frontier, max_gap_lines=window_merge_max_gap_lines
    )
    return RankedProjection(
        frontier=frontier,
        edges=edges,
        merged_windows=merged_windows,
        read_loop_guard=read_loop_guard,
        compatibility=profile_compatibility(edges, ranked, selected_profile),
        matrices=profile_matrix_bank(
            graph,
            selected_profile,
            frozenset(best_depth),
            ranked_node_ids=tuple(node.id for node in ranked),
            frontier_node_ids=tuple(entry.node.id for entry in frontier),
        ),
        ranked_ids=tuple(node.id for node in ranked),
    )


def build_path_projection(
    graph: TypedGraph,
    selected_profile: GraphProfile,
    seed_ids: tuple[str, ...],
    ranked_ids: tuple[str, ...],
    scores: Mapping[str, float],
    *,
    path_budget: int,
    path_max_hops: int,
) -> PathProjection:
    source_sink = source_sink_frontier(graph, selected_profile, seed_ids, ranked_ids)
    paths, backend, fallback_count, pair_count, candidate_count = (
        typed_paths_with_backend(
            graph,
            selected_profile,
            source_sink,
            scores,
            path_budget=path_budget,
            max_hops=path_max_hops,
        )
    )
    return PathProjection(
        source_sink,
        paths,
        flow_lite(paths),
        backend,
        fallback_count,
        pair_count,
        candidate_count,
    )


def _frontier_entries(
    profile: GraphProfile, ranked: Iterable[Node], scores: Mapping[str, float]
) -> tuple[FrontierEntry, ...]:
    entries: list[FrontierEntry] = []
    for node in ranked:
        action = frontier_action(profile, node)
        if action is None:
            continue
        entries.append(FrontierEntry(node, action, scores[node.id]))
    return tuple(entries)


def _ranked_edges(
    edges: Iterable[OrientedEdge], ranked: Iterable[Node]
) -> tuple[OrientedEdge, ...]:
    ranked_ids = {node.id for node in ranked}
    return tuple(
        edge
        for edge in edges
        if edge.source in ranked_ids and edge.target in ranked_ids
    )
