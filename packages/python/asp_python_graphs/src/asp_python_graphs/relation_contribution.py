"""Relation contribution explanations for graph turbo ranking."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable

from .model import Node, OrientedEdge


def graph_turbo_relation_reasons_by_node(
    edges: Iterable[OrientedEdge],
    ranked: Iterable[Node],
    *,
    max_relations: int = 3,
) -> dict[str, tuple[str, ...]]:
    ranked_ids = {node.id for node in ranked}
    relation_mass_by_node: dict[str, dict[str, float]] = defaultdict(dict)
    relation_edge_by_node: dict[str, dict[str, OrientedEdge]] = defaultdict(dict)
    for edge in edges:
        if edge.target not in ranked_ids or edge.weight <= 0.0:
            continue
        relation_mass = relation_mass_by_node[edge.target]
        relation_mass[edge.relation] = (
            relation_mass.get(edge.relation, 0.0) + edge.weight
        )
        relation_edges = relation_edge_by_node[edge.target]
        current = relation_edges.get(edge.relation)
        if current is None or edge.weight > current.weight:
            relation_edges[edge.relation] = edge
    return {
        node_id: tuple(
            reason
            for relation, weight in top_relations
            for reason in (
                f"relation:{relation}:{weight:+.2f}",
                _oriented_edge_reason(relation_edge_by_node[node_id][relation]),
            )
        )
        for node_id, relation_mass in relation_mass_by_node.items()
        for top_relations in (_top_relations(relation_mass, max_relations),)
    }


def _oriented_edge_reason(edge: OrientedEdge) -> str:
    direction = "reversed" if edge.reversed else "forward"
    return f"oriented:{edge.source}>{edge.target}:{edge.relation}:{direction}"


def _top_relations(
    relation_mass: dict[str, float], max_relations: int
) -> tuple[tuple[str, float], ...]:
    return tuple(
        sorted(relation_mass.items(), key=lambda item: (-item[1], item[0]))[
            :max_relations
        ]
    )
