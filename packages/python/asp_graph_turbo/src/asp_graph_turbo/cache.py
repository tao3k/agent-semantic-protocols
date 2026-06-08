"""Packet fingerprint and sparse graph cache for graph turbo."""

from __future__ import annotations

import hashlib
import json
from collections import OrderedDict
from collections.abc import Mapping

from .backend import SparseGraphBackend, build_sparse_backend
from .cache_store import (
    _load_persistent_backend,
    _persistent_entry_count,
    _store_persistent_backend,
)
from .model import GraphCache, GraphProfile, TypedGraph

_MAX_CACHE_ENTRIES = 16
_BACKEND_CACHE: OrderedDict[str, SparseGraphBackend] = OrderedDict()


def packet_fingerprint(
    graph: TypedGraph,
    profile: GraphProfile,
    *,
    seed_ids: tuple[str, ...],
    budget: int,
    kind_budgets: Mapping[str, int],
    path_budget: int,
    path_max_hops: int,
    window_merge_enabled: bool,
    window_merge_max_gap_lines: int,
) -> str:
    canonical = {
        "algorithm": "typed-ppr-diverse",
        "budget": budget,
        "kindBudgets": dict(sorted(kind_budgets.items())),
        "pathBudget": path_budget,
        "pathMaxHops": path_max_hops,
        "profile": {
            "name": profile.name,
            "kindBonus": dict(sorted(profile.kind_bonus.items())),
            "relationWeightMultiplier": dict(
                sorted(profile.relation_weight_multiplier.items())
            ),
        },
        "seedIds": list(seed_ids),
        "windowMerge": {
            "enabled": window_merge_enabled,
            "maxGapLines": window_merge_max_gap_lines,
        },
        "graph": {
            "nodes": [
                {
                    "id": node.id,
                    "kind": node.kind,
                    "role": node.role,
                    "value": node.value,
                    "action": node.action,
                    "weight": node.weight,
                    "fields": dict(sorted(node.fields.items())),
                }
                for node in sorted(graph.nodes.values(), key=lambda item: item.id)
            ],
            "edges": [
                {
                    "source": edge.source,
                    "target": edge.target,
                    "relation": edge.relation,
                    "weight": edge.weight,
                    "fields": dict(sorted(edge.fields.items())),
                }
                for edge in sorted(
                    graph.edges,
                    key=lambda item: (item.source, item.target, item.relation),
                )
            ],
        },
    }
    payload = json.dumps(canonical, sort_keys=True, separators=(",", ":"))
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


def cached_sparse_backend(
    graph: TypedGraph,
    profile: GraphProfile,
    fingerprint: str,
    *,
    enabled: bool,
) -> tuple[SparseGraphBackend, GraphCache]:
    if not enabled:
        backend = build_sparse_backend(graph, profile)
        return backend, GraphCache(
            fingerprint, "disabled", "scipy-csr", len(_BACKEND_CACHE)
        )
    cached = _BACKEND_CACHE.get(fingerprint)
    if cached is not None:
        _BACKEND_CACHE.move_to_end(fingerprint)
        return cached, GraphCache(fingerprint, "hit", "scipy-csr", len(_BACKEND_CACHE))
    persistent = _load_persistent_backend(fingerprint)
    if persistent is not None:
        _remember_backend(fingerprint, persistent)
        return persistent, GraphCache(
            fingerprint, "hit", "scipy-csr", _persistent_entry_count()
        )
    backend = build_sparse_backend(graph, profile)
    _remember_backend(fingerprint, backend)
    _store_persistent_backend(fingerprint, backend)
    return backend, GraphCache(fingerprint, "miss", "scipy-csr", len(_BACKEND_CACHE))


def _remember_backend(fingerprint: str, backend: SparseGraphBackend) -> None:
    _BACKEND_CACHE[fingerprint] = backend
    _BACKEND_CACHE.move_to_end(fingerprint)
    while len(_BACKEND_CACHE) > _MAX_CACHE_ENTRIES:
        _BACKEND_CACHE.popitem(last=False)
