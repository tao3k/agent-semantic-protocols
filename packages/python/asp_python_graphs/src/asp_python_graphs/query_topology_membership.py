# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Topology membership adjustments for graph turbo owner ranking."""

from __future__ import annotations

from collections import deque
from collections.abc import Mapping
from dataclasses import dataclass
from threading import RLock
from weakref import WeakKeyDictionary

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


@dataclass(frozen=True, slots=True)
class TopologyMembershipIndex:
    """Query-local topology facts compiled once from an immutable graph."""

    topology_node_ids: frozenset[str]
    neighbors: Mapping[str, tuple[str, ...]]


_INDEX_LOCK = RLock()
_INDEX_CACHE: WeakKeyDictionary[TypedGraph, tuple[int, TopologyMembershipIndex]] = (
    WeakKeyDictionary()
)


def build_topology_membership_index(graph: TypedGraph) -> TopologyMembershipIndex:
    with _INDEX_LOCK:
        cached = _INDEX_CACHE.get(graph)
        if cached is not None and cached[0] == graph.revision:
            return cached[1]
    topology_node_ids = _topology_node_ids(graph)
    neighbors: dict[str, list[str]] = {}
    if topology_node_ids:
        for edge in graph.edges:
            if edge.relation not in _TOPOLOGY_RELATIONS:
                continue
            neighbors.setdefault(edge.source, []).append(edge.target)
            neighbors.setdefault(edge.target, []).append(edge.source)
    index = TopologyMembershipIndex(
        topology_node_ids=topology_node_ids,
        neighbors={
            node_id: tuple(sorted(set(adjacent)))
            for node_id, adjacent in neighbors.items()
        },
    )
    with _INDEX_LOCK:
        _INDEX_CACHE[graph] = (graph.revision, index)
    return index


def topology_membership_adjustment(
    graph: TypedGraph,
    *,
    profile_name: str,
    node_id: str,
    index: TopologyMembershipIndex | None = None,
) -> float:
    if profile_name != "owner-query":
        return 0.0
    node = graph.nodes.get(node_id)
    if node is None or node.kind not in _OWNER_KINDS:
        return 0.0
    membership_index = index or build_topology_membership_index(graph)
    topology_node_ids = membership_index.topology_node_ids
    if not topology_node_ids:
        return 0.0
    if _direct_topology_membership(
        graph,
        node_id,
        topology_node_ids,
        membership_index.neighbors,
        local_only=True,
    ):
        return TOPOLOGY_MEMBERSHIP_BONUS
    if _nearby_topology_membership(
        graph,
        node_id,
        topology_node_ids,
        membership_index.neighbors,
        local_only=True,
    ):
        return TOPOLOGY_NEARBY_BONUS
    if _direct_topology_membership(
        graph, node_id, topology_node_ids, membership_index.neighbors
    ):
        return TOPOLOGY_NEARBY_BONUS
    if _nearby_topology_membership(
        graph, node_id, topology_node_ids, membership_index.neighbors
    ):
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
    neighbors: Mapping[str, tuple[str, ...]],
    *,
    local_only: bool = False,
) -> bool:
    for adjacent_id in neighbors.get(node_id, ()):
        if adjacent_id in topology_node_ids and _matches_topology_scope(
            graph, adjacent_id, local_only=local_only
        ):
            return True
    return False


def _nearby_topology_membership(
    graph: TypedGraph,
    node_id: str,
    topology_node_ids: frozenset[str],
    neighbors: Mapping[str, tuple[str, ...]],
    *,
    local_only: bool = False,
) -> bool:
    queue: deque[tuple[str, int]] = deque([(node_id, 0)])
    seen = {node_id}
    while queue:
        current_id, depth = queue.popleft()
        if depth >= 2:
            continue
        for next_id in neighbors.get(current_id, ()):
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
