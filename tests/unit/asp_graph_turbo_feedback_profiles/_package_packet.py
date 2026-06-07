"""Build/test/package packet fixture."""

from __future__ import annotations


def build_test_package_graph_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:cache", "kind": "query", "role": "term", "value": "cache"},
            {
                "id": "owner:cache",
                "kind": "owner",
                "role": "path",
                "value": "src/cache.py",
                "path": "src/cache.py",
            },
            {
                "id": "field:entries",
                "kind": "field",
                "role": "field",
                "value": "entries",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "entries",
                "locator": "src/cache.py:10:12",
            },
            {
                "id": "package:cache",
                "kind": "package",
                "role": "crate",
                "value": "cache-crate",
                "path": "Cargo.toml",
            },
            {
                "id": "build:cache-tests",
                "kind": "build",
                "role": "target",
                "value": "cargo test -p cache-crate",
            },
            {
                "id": "test:cache-unit",
                "kind": "test",
                "role": "path",
                "value": "tests/test_cache.py",
                "path": "tests/test_cache.py",
            },
            {
                "id": "dependency:serde",
                "kind": "dependency",
                "role": "pkg",
                "value": "serde",
            },
        ],
        "edges": [
            {"source": "q:cache", "target": "field:entries", "relation": "matches"},
            {
                "source": "owner:cache",
                "target": "field:entries",
                "relation": "contains",
            },
            {
                "source": "owner:cache",
                "target": "package:cache",
                "relation": "belongs_to",
            },
            {
                "source": "package:cache",
                "target": "build:cache-tests",
                "relation": "builds",
            },
            {
                "source": "build:cache-tests",
                "target": "test:cache-unit",
                "relation": "tests",
            },
            {
                "source": "package:cache",
                "target": "test:cache-unit",
                "relation": "tests",
            },
            {
                "source": "package:cache",
                "target": "dependency:serde",
                "relation": "depends_on",
            },
        ],
    }


def provider_bridge_package_graph_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:entries", "kind": "query", "role": "term", "value": "entries"},
            {
                "id": "field:entries",
                "kind": "field",
                "role": "field",
                "value": "entries",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "entries",
                "locator": "src/cache.py:10:12",
            },
            {
                "id": "package:cache",
                "kind": "package",
                "role": "package",
                "value": "cache-crate",
                "path": "Cargo.toml",
            },
            {
                "id": "build:cache-tests",
                "kind": "build",
                "role": "target",
                "value": "cargo test -p cache-crate",
            },
            {
                "id": "test:cache-unit",
                "kind": "test",
                "role": "path",
                "value": "tests/test_cache.py",
                "path": "tests/test_cache.py",
            },
            {
                "id": "dependency:serde",
                "kind": "dependency",
                "role": "pkg",
                "value": "serde",
            },
        ],
        "edges": [
            {"source": "q:entries", "target": "field:entries", "relation": "matches"},
            {"source": "field:entries", "target": "package:cache", "relation": "belongs_to"},
            {"source": "package:cache", "target": "build:cache-tests", "relation": "builds"},
            {"source": "build:cache-tests", "target": "test:cache-unit", "relation": "tests"},
            {"source": "package:cache", "target": "dependency:serde", "relation": "depends_on"},
        ],
    }
