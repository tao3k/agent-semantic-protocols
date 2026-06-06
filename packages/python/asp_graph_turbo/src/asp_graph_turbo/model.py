"""Data model for ASP graph turbo."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from typing import Any

from .policy import edge_weight_for


@dataclass(frozen=True)
class Node:
    id: str
    kind: str
    role: str
    value: str
    action: str | None = None
    weight: float = 1.0
    fields: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class Edge:
    source: str
    target: str
    relation: str
    weight: float = 1.0
    fields: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True, order=True)
class AllowedTransition:
    source_kind: str
    target_kind: str


@dataclass(frozen=True)
class GraphProfile:
    name: str
    allowed_relations: frozenset[str]
    allowed_transitions: frozenset[AllowedTransition]
    frontier_actions: Mapping[str, str]
    kind_bonus: Mapping[str, float] = field(default_factory=dict)
    max_depth: int = 2


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
class ProfileCompatibility:
    profile: str
    compatible: bool
    allowed_relations: tuple[str, ...]
    allowed_transitions: tuple[AllowedTransition, ...]
    kind_bonus: Mapping[str, float]
    frontier_actions: Mapping[str, str]


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
class AlgorithmMetrics:
    node_count: int
    edge_count: int
    selected_edge_count: int
    reachable_node_count: int
    ranked_node_count: int
    path_count: int
    merged_window_count: int
    cache_status: str


@dataclass(frozen=True)
class GraphResult:
    profile: GraphProfile
    seed_ids: tuple[str, ...]
    ranked_nodes: tuple[Node, ...]
    frontier: tuple[FrontierEntry, ...]
    scores: Mapping[str, float]
    selected_edges: tuple[Edge, ...]
    budget: int
    kind_budgets: Mapping[str, int]
    merged_windows: tuple[MergedWindow, ...]
    profile_compatibility: tuple[ProfileCompatibility, ...]
    source_sink_frontier: SourceSinkFrontier
    typed_paths: tuple[TypedPath, ...]
    flow_lite: FlowLite
    packet_fingerprint: str
    graph_cache: GraphCache
    algorithm_trace: tuple[AlgorithmTraceStep, ...]
    rank_explanations: tuple[RankExplanation, ...]
    algorithm_metrics: AlgorithmMetrics
    profiles: tuple[str, ...]
    omit: tuple[str, ...]
    avoid: tuple[str, ...]


class TypedGraph:
    """A small typed heterogeneous graph built from schema-backed ASP facts."""

    def __init__(self, nodes: Iterable[Node] = (), edges: Iterable[Edge] = ()) -> None:
        self.nodes: dict[str, Node] = {}
        self.edges: list[Edge] = []
        self._outgoing: dict[str, list[Edge]] = defaultdict(list)
        self._incoming: dict[str, list[Edge]] = defaultdict(list)
        for node in nodes:
            self.add_node(node)
        for edge in edges:
            self.add_edge(edge)

    def add_node(self, node: Node) -> None:
        if not node.id:
            raise ValueError("node id must not be empty")
        self.nodes[node.id] = node

    def add_edge(self, edge: Edge) -> None:
        if edge.source not in self.nodes:
            raise ValueError(f"edge source missing from graph: {edge.source}")
        if edge.target not in self.nodes:
            raise ValueError(f"edge target missing from graph: {edge.target}")
        self.edges.append(edge)
        self._outgoing[edge.source].append(edge)
        self._incoming[edge.target].append(edge)

    def adjacent_edges(self, node_id: str, allowed_relations: frozenset[str]) -> list[Edge]:
        return [
            edge
            for edge in [*self._outgoing[node_id], *self._incoming[node_id]]
            if edge.relation in allowed_relations
        ]

    @classmethod
    def from_packet(cls, packet: Mapping[str, Any]) -> "TypedGraph":
        graph = packet.get("graph")
        if isinstance(graph, Mapping):
            source = graph
        else:
            source = packet
        nodes = tuple(_node_from_mapping(item) for item in _list_field(source, "nodes"))
        edges = tuple(_edge_from_mapping(item) for item in _list_field(source, "edges"))
        return cls(nodes, edges)


def _list_field(source: Mapping[str, Any], name: str) -> list[Mapping[str, Any]]:
    value = source.get(name, [])
    if not isinstance(value, list):
        raise ValueError(f"{name} must be a list")
    if not all(isinstance(item, Mapping) for item in value):
        raise ValueError(f"{name} entries must be objects")
    return value


def _node_from_mapping(item: Mapping[str, Any]) -> Node:
    node_id = _string_field(item, "id")
    kind = _string_field(item, "kind")
    role = str(item.get("role") or kind)
    value = str(item.get("value") or node_id)
    action = item.get("action")
    if action is not None:
        action = str(action)
    weight = float(item.get("weight", 1.0))
    fields = {
        key: value
        for key, value in item.items()
        if key not in {"id", "kind", "role", "value", "action", "weight"}
    }
    return Node(node_id, kind, role, value, action, weight, fields)


def _edge_from_mapping(item: Mapping[str, Any]) -> Edge:
    source = _string_field(item, "source")
    target = _string_field(item, "target")
    relation = _string_field(item, "relation")
    weight = edge_weight_for(relation, item.get("weight"))
    fields = {
        key: value
        for key, value in item.items()
        if key not in {"source", "target", "relation", "weight"}
    }
    return Edge(source, target, relation, weight, fields)


def _string_field(item: Mapping[str, Any], name: str) -> str:
    value = item.get(name)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{name} must be a non-empty string")
    return value
