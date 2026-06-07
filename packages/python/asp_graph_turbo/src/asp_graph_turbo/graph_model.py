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
        graph = packet.get("graph")
        source = graph if isinstance(graph, Mapping) else packet
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
