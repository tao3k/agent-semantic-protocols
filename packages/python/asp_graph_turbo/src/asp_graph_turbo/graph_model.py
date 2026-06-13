"""Typed graph primitives for ASP graph turbo."""

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


@dataclass(frozen=True)
class OrientedEdge:
    source: str
    target: str
    relation: str
    original_source: str
    original_target: str
    reversed: bool
    weight: float = 1.0
    fields: Mapping[str, Any] = field(default_factory=dict)


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

    def adjacent_edges(
        self, node_id: str, allowed_relations: frozenset[str]
    ) -> list[Edge]:
        return [
            edge
            for edge in [*self._outgoing[node_id], *self._incoming[node_id]]
            if edge.relation in allowed_relations
        ]

    @classmethod
    def from_packet(cls, packet: Mapping[str, Any]) -> "TypedGraph":
        sources = _graph_sources(packet)
        nodes = tuple(
            _node_from_mapping(item)
            for source in sources
            for item in [*_list_field(source, "nodes"), *_gap_node_mappings(source)]
        )
        edges = tuple(
            _edge_from_mapping(item)
            for source in sources
            for item in [*_list_field(source, "edges"), *_gap_edge_mappings(source)]
        )
        return cls(nodes, edges)


def _graph_sources(packet: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    graph = packet.get("graph")
    if isinstance(graph, Mapping):
        return (graph,)
    if graph is not None:
        raise ValueError("graph must be an object")
    graphs = packet.get("graphs")
    if graphs is None:
        return (packet,)
    if not isinstance(graphs, list):
        raise ValueError("graphs must be a list")
    if not all(isinstance(item, Mapping) for item in graphs):
        raise ValueError("graphs entries must be objects")
    return tuple(graphs)


def _list_field(source: Mapping[str, Any], name: str) -> list[Mapping[str, Any]]:
    value = source.get(name, [])
    if not isinstance(value, list):
        raise ValueError(f"{name} must be a list")
    if not all(isinstance(item, Mapping) for item in value):
        raise ValueError(f"{name} entries must be objects")
    return value


def _optional_list_field(source: Mapping[str, Any], name: str) -> list[Mapping[str, Any]]:
    value = source.get(name, [])
    if value is None:
        return []
    if not isinstance(value, list):
        raise ValueError(f"{name} must be a list")
    if not all(isinstance(item, Mapping) for item in value):
        raise ValueError(f"{name} entries must be objects")
    return value


def _gap_node_mappings(source: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    return [_gap_node_mapping(item) for item in _optional_list_field(source, "gaps")]


def _gap_edge_mappings(source: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    owner_by_path = {
        owner_path: _string_field(item, "id")
        for item in _list_field(source, "nodes")
        if item.get("kind") == "owner"
        for owner_path in (_owner_path_for_mapping(item),)
        if owner_path is not None
    }
    edges: list[Mapping[str, Any]] = []
    for item in _optional_list_field(source, "gaps"):
        owner_path = _optional_string_field(item, "ownerPath")
        if owner_path is None or owner_path not in owner_by_path:
            continue
        edges.append(
            {
                "source": owner_by_path[owner_path],
                "target": _gap_id(item),
                "relation": "requires-evidence",
                "fields": {"provenance": "parser", "confidence": "high"},
            }
        )
    return edges


def _gap_node_mapping(item: Mapping[str, Any]) -> Mapping[str, Any]:
    owner_path = _optional_string_field(item, "ownerPath")
    severity = _optional_string_field(item, "severity")
    fields = {"source": "evidence-gap"}
    if severity is not None:
        fields["severity"] = severity
    node: dict[str, Any] = {
        "id": _gap_id(item),
        "kind": "evidence-gap",
        "role": "gap",
        "value": _string_field(item, "summary"),
        "action": "evidence",
        "fields": fields,
    }
    if owner_path is not None:
        node["path"] = owner_path
        node["ownerPath"] = owner_path
    return node


def _gap_id(item: Mapping[str, Any]) -> str:
    value = item.get("gapId") or item.get("id")
    if not isinstance(value, str) or not value:
        raise ValueError("gapId must be a non-empty string")
    return value


def _owner_path_for_mapping(item: Mapping[str, Any]) -> str | None:
    for name in ("ownerPath", "path", "value"):
        value = _optional_string_field(item, name)
        if value is not None:
            return value
    return None


def _optional_string_field(item: Mapping[str, Any], name: str) -> str | None:
    value = item.get(name)
    return value if isinstance(value, str) and value else None


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
    fields = {
        key: value
        for key, value in item.items()
        if key not in {"source", "target", "relation", "weight"}
    }
    weight = edge_weight_for(relation, item.get("weight"), fields)
    return Edge(source, target, relation, weight, fields)


def _string_field(item: Mapping[str, Any], name: str) -> str:
    value = item.get(name)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{name} must be a non-empty string")
    return value
