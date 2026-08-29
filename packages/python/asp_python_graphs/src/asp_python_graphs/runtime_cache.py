"""Runtime caches for repeated graph-turbo ranking computations."""

from __future__ import annotations

from collections import OrderedDict
from collections.abc import Mapping
from typing import TypeVar

from .backend import SparseGraphBackend, multi_source_hop_lengths, reachable_edges
from .model import OrientedEdge
from .pagerank import (
    GraphTurboPprResult,
    graph_turbo_typed_personalized_pagerank_result,
)

_MAX_RUNTIME_CACHE_ENTRIES = 64
_DEPTH_CACHE: OrderedDict[tuple[str, tuple[str, ...], int], dict[str, int]] = (
    OrderedDict()
)
_PPR_CACHE: OrderedDict[
    tuple[str, tuple[str, ...], tuple[tuple[str, float], ...], float, int, float],
    GraphTurboPprResult,
] = OrderedDict()
_REACHABLE_EDGE_CACHE: OrderedDict[
    tuple[str, tuple[str, ...], int],
    dict[tuple[str, str, str], OrientedEdge],
] = OrderedDict()

_CacheKey = TypeVar("_CacheKey")
_CacheValue = TypeVar("_CacheValue")


def cached_hop_lengths(
    backend_key: str,
    backend: SparseGraphBackend,
    seed_ids: tuple[str, ...],
    max_depth: int,
    *,
    enabled: bool,
) -> tuple[dict[str, int], str]:
    if not enabled:
        return multi_source_hop_lengths(backend, seed_ids, max_depth), "disabled"
    key = (backend_key, seed_ids, max_depth)
    cached = _DEPTH_CACHE.get(key)
    if cached is not None:
        _DEPTH_CACHE.move_to_end(key)
        return dict(cached), "hit"
    result = multi_source_hop_lengths(backend, seed_ids, max_depth)
    _remember_cache_entry(_DEPTH_CACHE, key, dict(result))
    return result, "miss"


def cached_pagerank(
    backend_key: str,
    backend: SparseGraphBackend,
    seed_ids: tuple[str, ...],
    seed_weights: Mapping[str, float],
    *,
    enabled: bool,
) -> tuple[GraphTurboPprResult, str]:
    if not enabled:
        return (
            graph_turbo_typed_personalized_pagerank_result(
                backend,
                seed_ids,
                seed_weights=seed_weights,
            ),
            "disabled",
        )
    key = (
        backend_key,
        seed_ids,
        tuple(
            sorted((node_id, float(weight)) for node_id, weight in seed_weights.items())
        ),
        0.85,
        100,
        1e-9,
    )
    cached = _PPR_CACHE.get(key)
    if cached is not None:
        _PPR_CACHE.move_to_end(key)
        return cached, "hit"
    result = graph_turbo_typed_personalized_pagerank_result(
        backend,
        seed_ids,
        seed_weights=seed_weights,
    )
    _remember_cache_entry(_PPR_CACHE, key, result)
    return result, "miss"


def cached_reachable_edges(
    backend_key: str,
    backend: SparseGraphBackend,
    seed_ids: tuple[str, ...],
    max_depth: int,
    best_depth: Mapping[str, int],
    *,
    enabled: bool,
) -> tuple[dict[tuple[str, str, str], OrientedEdge], str]:
    if not enabled:
        return reachable_edges(backend, best_depth), "disabled"
    key = (backend_key, seed_ids, max_depth)
    cached = _REACHABLE_EDGE_CACHE.get(key)
    if cached is not None:
        _REACHABLE_EDGE_CACHE.move_to_end(key)
        return dict(cached), "hit"
    result = reachable_edges(backend, best_depth)
    _remember_cache_entry(_REACHABLE_EDGE_CACHE, key, dict(result))
    return result, "miss"


def _remember_cache_entry(
    cache: OrderedDict[_CacheKey, _CacheValue],
    key: _CacheKey,
    value: _CacheValue,
) -> None:
    cache[key] = value
    cache.move_to_end(key)
    while len(cache) > _MAX_RUNTIME_CACHE_ENTRIES:
        cache.popitem(last=False)
