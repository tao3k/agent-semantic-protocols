"""Topology membership adjustments for graph turbo owner ranking."""

from __future__ import annotations

from collections import deque

from .model import Node, TypedGraph

TOPOLOGY_MEMBERSHIP_BONUS = 0.45
TOPOLOGY_NEARBY_BONUS = 0.2
TOPOLOGY_DRIFT_PENALTY = 0.25

_OWNER_KINDS = frozenset({"owner"})
_TOPOLOGY_NODE_KINDS = frozenset(
    {
        "workspace",
        "submodule",
        "provider-root",
        "package",
        "project",
        "module-root",
    }
)
_BROAD_TOPOLOGY_NODE_KINDS = frozenset({"workspace", "provider-root"})
_BROAD_TOPOLOGY_ROLES = frozenset({"root", "language-root"})
_TOPOLOGY_RELATIONS = frozenset(
    {
        "contains",
        "has_provider_root",
        "has_submodule",
        "member",
        "owns",
        "indexes",
    }
)


def topology_membership_adjustment(
    graph: TypedGraph,
    *,
    profile_name: str,
    node_id: str,
) -> float:
    if profile_name != "owner-query":
        return 0.0
    node = graph.nodes.get(node_id)
    if node is None or node.kind not in _OWNER_KINDS:
        return 0.0
    topology_node_ids = _topology_node_ids(graph)
    if not topology_node_ids:
        return 0.0
    if _direct_topology_membership(
        graph,
        node_id,
        topology_node_ids,
        local_only=True,
    ):
        return TOPOLOGY_MEMBERSHIP_BONUS
    if _nearby_topology_membership(
        graph,
        node_id,
        topology_node_ids,
        local_only=True,
    ):
        return TOPOLOGY_NEARBY_BONUS
    if _direct_topology_membership(graph, node_id, topology_node_ids):
        return TOPOLOGY_NEARBY_BONUS
    if _nearby_topology_membership(graph, node_id, topology_node_ids):
        return TOPOLOGY_NEARBY_BONUS
    return -TOPOLOGY_DRIFT_PENALTY


def _topology_node_ids(graph: TypedGraph) -> frozenset[str]:
    return frozenset(
        node.id
        for node in graph.nodes.values()
        if node.kind in _TOPOLOGY_NODE_KINDS or _has_topology_role(node)
    )


def _has_topology_role(node: Node) -> bool:
    role = node.role.lower()
    return role in _TOPOLOGY_NODE_KINDS or role in {"workspace-member", "language-root"}


def _direct_topology_membership(
    graph: TypedGraph,
    node_id: str,
    topology_node_ids: frozenset[str],
    *,
    local_only: bool = False,
) -> bool:
    for edge in graph.edges:
        if edge.relation not in _TOPOLOGY_RELATIONS:
            continue
        if (
            edge.source == node_id
            and edge.target in topology_node_ids
            and _matches_topology_scope(graph, edge.target, local_only=local_only)
        ):
            return True
        if (
            edge.target == node_id
            and edge.source in topology_node_ids
            and _matches_topology_scope(graph, edge.source, local_only=local_only)
        ):
            return True
    return False


def _nearby_topology_membership(
    graph: TypedGraph,
    node_id: str,
    topology_node_ids: frozenset[str],
    *,
    local_only: bool = False,
) -> bool:
    queue: deque[tuple[str, int]] = deque([(node_id, 0)])
    seen = {node_id}
    while queue:
        current_id, depth = queue.popleft()
        if depth >= 2:
            continue
        for edge in graph.edges:
            if edge.relation not in _TOPOLOGY_RELATIONS:
                continue
            if edge.source == current_id:
                next_id = edge.target
            elif edge.target == current_id:
                next_id = edge.source
            else:
                continue
            if next_id in seen:
                continue
            if next_id in topology_node_ids and _matches_topology_scope(
                graph,
                next_id,
                local_only=local_only,
            ):
                return True
            seen.add(next_id)
            queue.append((next_id, depth + 1))
    return False


def _matches_topology_scope(
    graph: TypedGraph,
    node_id: str,
    *,
    local_only: bool,
) -> bool:
    if not local_only:
        return True
    node = graph.nodes.get(node_id)
    if node is None:
        return False
    return not _is_broad_topology_node(node)


def _is_broad_topology_node(node: Node) -> bool:
    return (
        node.kind in _BROAD_TOPOLOGY_NODE_KINDS
        or node.role.lower() in _BROAD_TOPOLOGY_ROLES
    )
