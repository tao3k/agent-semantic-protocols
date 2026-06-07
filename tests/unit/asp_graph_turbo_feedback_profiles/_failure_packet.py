"""Failure evidence packet fixture."""

from __future__ import annotations


def failure_evidence_graph_packet() -> dict[str, object]:
    return {
        "nodes": [
            {
                "id": "failure:cache",
                "kind": "failure",
                "role": "test-failure",
                "value": "cache replay missed",
            },
            {
                "id": "assert:replay",
                "kind": "assert",
                "role": "assertion",
                "value": "expected hit actual miss",
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
                "id": "collection:entries",
                "kind": "collection",
                "role": "sequence",
                "value": "Vec<Entry>",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "entries",
                "locator": "src/cache.py:10:12",
            },
            {
                "id": "type:entry",
                "kind": "type",
                "role": "type",
                "value": "Entry",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "Entry",
                "locator": "src/cache.py:4:8",
            },
            {
                "id": "hot:write",
                "kind": "hot",
                "role": "fn",
                "value": "write_entries",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "write_entries",
                "locator": "src/cache.py:30:44",
            },
            {
                "id": "evidence:file-hash",
                "kind": "evidence",
                "role": "signal",
                "value": "file_hash changed",
            },
            {
                "id": "test:cache",
                "kind": "test",
                "role": "path",
                "value": "tests/test_cache.py",
                "path": "tests/test_cache.py",
            },
        ],
        "edges": [
            {
                "source": "failure:cache",
                "target": "assert:replay",
                "relation": "explains",
            },
            {"source": "failure:cache", "target": "test:cache", "relation": "fails"},
            {"source": "assert:replay", "target": "hot:write", "relation": "checks"},
            {
                "source": "assert:replay",
                "target": "field:entries",
                "relation": "checks",
            },
            {
                "source": "field:entries",
                "target": "collection:entries",
                "relation": "collection_of",
            },
            {
                "source": "field:entries",
                "target": "type:entry",
                "relation": "has_type",
            },
            {
                "source": "hot:write",
                "target": "evidence:file-hash",
                "relation": "validates",
            },
        ],
    }
