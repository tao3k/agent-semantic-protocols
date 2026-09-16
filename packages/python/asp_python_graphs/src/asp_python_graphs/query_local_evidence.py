# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Owner-local topology evidence adjustments for graph turbo ranking."""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping
from dataclasses import dataclass
from threading import RLock
from weakref import WeakKeyDictionary

from .model import TypedGraph

_LOCAL_EVIDENCE_NODE_KINDS = frozenset({"hot", "item", "syntax", "test"})
_LOCAL_EVIDENCE_RELATIONS = frozenset({"contains", "covers", "requires-evidence"})
_LOCAL_EVIDENCE_BONUS = 0.35
_PATH_ONLY_OWNER_PENALTY = 0.2


@dataclass(frozen=True, slots=True)
class LocalEvidenceIndex:
    """Query-local owner evidence counts compiled once from graph edges."""

    local_kind_count: Mapping[str, int]
    relation_count: Mapping[str, int]


_INDEX_LOCK = RLock()
_INDEX_CACHE: WeakKeyDictionary[TypedGraph, tuple[int, LocalEvidenceIndex]] = (
    WeakKeyDictionary()
)


def build_local_evidence_index(graph: TypedGraph) -> LocalEvidenceIndex:
    with _INDEX_LOCK:
        cached = _INDEX_CACHE.get(graph)
        if cached is not None and cached[0] == graph.revision:
            return cached[1]
    local_kind_count: Counter[str] = Counter()
    relation_count: Counter[str] = Counter()
    for edge in graph.edges:
        source = graph.nodes.get(edge.source)
        target = graph.nodes.get(edge.target)
        for owner_id, adjacent in ((edge.source, target), (edge.target, source)):
            owner = graph.nodes.get(owner_id)
            if owner is None or owner.kind != "owner":
                continue
            if adjacent is not None and adjacent.kind in _LOCAL_EVIDENCE_NODE_KINDS:
                local_kind_count[owner_id] += 1
            if edge.relation in _LOCAL_EVIDENCE_RELATIONS:
                relation_count[owner_id] += 1
    index = LocalEvidenceIndex(
        local_kind_count=dict(local_kind_count),
        relation_count=dict(relation_count),
    )
    with _INDEX_LOCK:
        _INDEX_CACHE[graph] = (graph.revision, index)
    return index


def local_evidence_adjustment(
    graph: TypedGraph,
    *,
    profile_name: str,
    node_id: str,
    index: LocalEvidenceIndex | None = None,
) -> float:
    if profile_name != "owner-query":
        return 0.0
    node = graph.nodes.get(node_id)
    if node is None or node.kind != "owner":
        return 0.0
    evidence_index = index or build_local_evidence_index(graph)
    local_kind_count = evidence_index.local_kind_count.get(node_id, 0)
    relation_count = evidence_index.relation_count.get(node_id, 0)
    if local_kind_count >= 2 or relation_count >= 2:
        return _LOCAL_EVIDENCE_BONUS
    if local_kind_count == 0 and relation_count == 0:
        return -_PATH_ONLY_OWNER_PENALTY
    return 0.0
