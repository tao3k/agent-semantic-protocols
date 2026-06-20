"""Owner-local topology evidence adjustments for graph turbo ranking."""

from __future__ import annotations

from .model import TypedGraph

_LOCAL_EVIDENCE_NODE_KINDS = frozenset({"hot", "item", "syntax", "test"})
_LOCAL_EVIDENCE_RELATIONS = frozenset({"contains", "covers", "requires-evidence"})
_LOCAL_EVIDENCE_BONUS = 0.35
_PATH_ONLY_OWNER_PENALTY = 0.2


def local_evidence_adjustment(
    graph: TypedGraph,
    *,
    profile_name: str,
    node_id: str,
) -> float:
    if profile_name != "owner-query":
        return 0.0
    node = graph.nodes.get(node_id)
    if node is None or node.kind != "owner":
        return 0.0
    local_kind_count = 0
    relation_count = 0
    for edge in graph.edges:
        if edge.source != node_id and edge.target != node_id:
            continue
        other_id = edge.target if edge.source == node_id else edge.source
        other = graph.nodes.get(other_id)
        if other is not None and other.kind in _LOCAL_EVIDENCE_NODE_KINDS:
            local_kind_count += 1
        if edge.relation in _LOCAL_EVIDENCE_RELATIONS:
            relation_count += 1
    if local_kind_count >= 2 or relation_count >= 2:
        return _LOCAL_EVIDENCE_BONUS
    if local_kind_count == 0 and relation_count == 0:
        return -_PATH_ONLY_OWNER_PENALTY
    return 0.0
