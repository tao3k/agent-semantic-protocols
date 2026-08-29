"""Persistent sparse backend cache storage for graph turbo."""

from __future__ import annotations

import json
import os
from collections.abc import Mapping
from pathlib import Path

from scipy.sparse import load_npz, save_npz

from .backend import SparseGraphBackend, sparse_backend_from_parts
from .model import OrientedEdge

_CACHE_NAMESPACE = "agent-semantic-protocol/graph-turbo"


def graph_cache_status() -> dict[str, object]:
    root = _cache_root()
    entries = _persistent_entries(root)
    return {
        "root": str(root) if root is not None else None,
        "enabled": root is not None,
        "entries": len(entries),
        "bytes": sum(_entry_bytes(entry) for entry in entries),
        "memoryEntries": 0,
    }


def invalidate_graph_cache() -> int:
    return sum(_remove_entry(entry) for entry in _persistent_entries(_cache_root()))


def prune_graph_cache(max_entries: int) -> int:
    if max_entries < 0:
        raise ValueError("max_entries must be non-negative")
    entries = sorted(
        _persistent_entries(_cache_root()),
        key=lambda entry: entry.metadata.stat().st_mtime,
        reverse=True,
    )
    return sum(_remove_entry(entry) for entry in entries[max_entries:])


def _load_persistent_backend(fingerprint: str) -> SparseGraphBackend | None:
    paths = _persistent_backend_paths(fingerprint)
    if paths is None or not paths.metadata.exists() or not paths.matrix.exists():
        return None
    try:
        metadata = json.loads(paths.metadata.read_text(encoding="utf-8"))
        node_ids = tuple(_string_items(metadata["nodeIds"]))
        selected_edges = tuple(
            _edge_from_mapping(item) for item in metadata["selectedEdges"]
        )
        adjacency = load_npz(paths.matrix)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError):
        return None
    return sparse_backend_from_parts(node_ids, adjacency, selected_edges)


def _store_persistent_backend(fingerprint: str, backend: SparseGraphBackend) -> None:
    paths = _persistent_backend_paths(fingerprint)
    if paths is None:
        return
    try:
        paths.directory.mkdir(parents=True, exist_ok=True)
        save_npz(paths.matrix, backend.adjacency)
        payload = {
            "fingerprint": fingerprint,
            "nodeIds": list(backend.node_ids),
            "selectedEdges": [
                _edge_to_mapping(edge) for edge in backend.selected_edges
            ],
        }
        paths.metadata.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    except OSError:
        return


def _persistent_entry_count() -> int:
    return len(_persistent_entries(_cache_root()))


def _persistent_backend_paths(fingerprint: str) -> "_PersistentBackendPaths | None":
    root = _cache_root()
    if root is None:
        return None
    key = fingerprint.replace(":", "-")
    return _PersistentBackendPaths(root, root / f"{key}.json", root / f"{key}.npz")


def _persistent_entries(root: Path | None) -> list["_PersistentBackendPaths"]:
    if root is None or not root.exists():
        return []
    return [
        _PersistentBackendPaths(root, metadata, metadata.with_suffix(".npz"))
        for metadata in root.glob("*.json")
        if metadata.with_suffix(".npz").exists()
    ]


def _entry_bytes(entry: "_PersistentBackendPaths") -> int:
    return entry.metadata.stat().st_size + entry.matrix.stat().st_size


def _remove_entry(entry: "_PersistentBackendPaths") -> int:
    return sum(1 for path in (entry.metadata, entry.matrix) if _unlink_existing(path))


def _unlink_existing(path: Path) -> bool:
    try:
        path.unlink()
    except FileNotFoundError:
        return False
    return True


def _cache_root() -> Path | None:
    project_cache = os.environ.get("PRJ_CACHE_HOME")
    if project_cache:
        return Path(project_cache) / _CACHE_NAMESPACE
    git_root = _find_git_root(Path.cwd())
    if git_root is None:
        return None
    return git_root / ".cache" / _CACHE_NAMESPACE


def _find_git_root(start: Path) -> Path | None:
    for candidate in (start, *start.parents):
        if (candidate / ".git").exists():
            return candidate
    return None


class _PersistentBackendPaths:
    def __init__(self, directory: Path, metadata: Path, matrix: Path) -> None:
        self.directory = directory
        self.metadata = metadata
        self.matrix = matrix


def _edge_to_mapping(edge: OrientedEdge) -> dict[str, object]:
    return {
        "source": edge.source,
        "target": edge.target,
        "relation": edge.relation,
        "weight": edge.weight,
        "originalSource": edge.original_source,
        "originalTarget": edge.original_target,
        "reversed": edge.reversed,
        "fields": dict(edge.fields),
    }


def _edge_from_mapping(item: Mapping[str, object]) -> OrientedEdge:
    source = str(item["source"])
    target = str(item["target"])
    original_source = str(item.get("originalSource", source))
    original_target = str(item.get("originalTarget", target))
    return OrientedEdge(
        source=str(item["source"]),
        target=str(item["target"]),
        relation=str(item["relation"]),
        original_source=original_source,
        original_target=original_target,
        reversed=bool(
            item.get("reversed", original_source != source or original_target != target)
        ),
        weight=float(item["weight"]),
        fields=dict(item.get("fields", {})),
    )


def _string_items(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise TypeError("expected string list")
    return tuple(str(item) for item in value)
