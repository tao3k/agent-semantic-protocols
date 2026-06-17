"""Ranking result model for ASP graph turbo."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

from .graph_model import Node, OrientedEdge
from .profile_model import GraphProfile, ProfileCompatibility, ProfileMatrixSummary


@dataclass(frozen=True)
class FrontierEntry:
    node: Node
    action: str
    score: float


@dataclass(frozen=True)
class MergedWindow:
    path: str
    start_line: int
    end_line: int
    node_ids: tuple[str, ...]


@dataclass(frozen=True)
class SourceSinkFrontier:
    source_ids: tuple[str, ...]
    sink_ids: tuple[str, ...]


@dataclass(frozen=True)
class TypedPath:
    id: str
    path_kind: str
    source: str
    sink: str
    node_ids: tuple[str, ...]
    relations: tuple[str, ...]
    cost: float
    score: float
    rank: int


@dataclass(frozen=True)
class FlowLite:
    ranked_path_ids: tuple[str, ...]


@dataclass(frozen=True)
class GraphCache:
    key: str
    status: str
    backend: str
    entries: int


@dataclass(frozen=True)
class AlgorithmTraceStep:
    step: str
    engine: str
    fields: Mapping[str, int | float | str | bool]


@dataclass(frozen=True)
class RankExplanation:
    node_id: str
    score: float
    depth: int
    reasons: tuple[str, ...]


@dataclass(frozen=True)
class ReceiptAdjustment:
    node_id: str
    effect: str
    score_delta: float
    reason: str


@dataclass(frozen=True)
class ReadMemoryProjection:
    seen_selectors: tuple[str, ...]
    suppressed_selectors: tuple[str, ...]


@dataclass(frozen=True)
class ReadLoopGuard:
    direct_code_action_count: int
    duplicate_selector_count: int
    adjacent_range_window_count: int
    same_owner_scan_count: int
    avoid: tuple[str, ...]


@dataclass(frozen=True)
class AlgorithmMetrics:
    node_count: int
    edge_count: int
    selected_edge_count: int
    reachable_node_count: int
    ranked_node_count: int
    path_count: int
    merged_window_count: int
    cache_status: str
    path_backend: str = "python-bfs-small"
    path_fallback_count: int = 0
    path_pair_count: int = 0
    path_candidate_count: int = 0
    read_loop_direct_code_action_count: int = 0
    read_loop_duplicate_selector_count: int = 0
    read_loop_adjacent_range_window_count: int = 0
    read_loop_same_owner_scan_count: int = 0
    read_memory_suppressed_count: int = 0
    receipt_boost_count: int = 0
    receipt_penalty_count: int = 0
    relation_channel_count: int = 0
    ppr_iterations: int = 0
    ppr_residual: float = 0.0
    ppr_dangling_mass_last: float = 0.0
    ppr_mass_sum: float = 0.0
    read_loop_second_pass_suppressed_count: int = 0
    read_loop_duplicate_selector_suppressed_count: int = 0
    read_loop_adjacent_range_merged_count: int = 0
    read_loop_same_owner_suppressed_count: int = 0
    query_seed_prior_count: int = 0
    query_seed_prior_mass: float = 0.0
    query_package_cohesion_count: int = 0
    query_package_drift_penalty_count: int = 0
    query_package_cohesion_delta: float = 0.0
    query_clause_coverage_count: int = 0
    query_clause_coverage_delta: float = 0.0


@dataclass(frozen=True)
class GraphResult:
    profile: GraphProfile
    seed_ids: tuple[str, ...]
    ranked_nodes: tuple[Node, ...]
    frontier: tuple[FrontierEntry, ...]
    scores: Mapping[str, float]
    selected_edges: tuple[OrientedEdge, ...]
    budget: int
    kind_budgets: Mapping[str, int]
    merged_windows: tuple[MergedWindow, ...]
    profile_compatibility: tuple[ProfileCompatibility, ...]
    profile_matrices: tuple[ProfileMatrixSummary, ...]
    source_sink_frontier: SourceSinkFrontier
    typed_paths: tuple[TypedPath, ...]
    flow_lite: FlowLite
    packet_fingerprint: str
    graph_cache: GraphCache
    algorithm_trace: tuple[AlgorithmTraceStep, ...]
    rank_explanations: tuple[RankExplanation, ...]
    receipt_adjustments: tuple[ReceiptAdjustment, ...]
    read_memory: ReadMemoryProjection
    algorithm_metrics: AlgorithmMetrics
    profiles: tuple[str, ...]
    omit: tuple[str, ...]
    avoid: tuple[str, ...]
