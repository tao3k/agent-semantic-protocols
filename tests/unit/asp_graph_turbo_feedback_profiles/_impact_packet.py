"""Impact profile packet fixture."""

from __future__ import annotations


def impact_graph_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:impact", "kind": "query", "role": "term", "value": "entries"},
            {
                "id": "q:collection",
                "kind": "query",
                "role": "term",
                "value": "collection mutation",
            },
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
                "value": "entries: Vec<Entry>",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "entries",
                "locator": "src/cache.py:10:12",
            },
            {
                "id": "type:vec-entry",
                "kind": "type",
                "role": "type",
                "value": "Vec<Entry>",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "Vec<Entry>",
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
                "id": "hot:mutate",
                "kind": "hot",
                "role": "fn",
                "value": "mutate_entries",
                "path": "src/cache.py",
                "ownerPath": "src/cache.py",
                "symbol": "mutate_entries",
                "locator": "src/cache.py:46:58",
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
            {"source": "q:impact", "target": "field:entries", "relation": "matches"},
            {
                "source": "q:collection",
                "target": "collection:entries",
                "relation": "matches",
            },
            {
                "source": "owner:cache",
                "target": "field:entries",
                "relation": "contains",
            },
            {
                "source": "field:entries",
                "target": "type:vec-entry",
                "relation": "has_type",
            },
            {
                "source": "field:entries",
                "target": "collection:entries",
                "relation": "collection_of",
            },
            {
                "source": "field:entries",
                "target": "hot:write",
                "relation": "relates",
            },
            {
                "source": "collection:entries",
                "target": "hot:mutate",
                "relation": "relates",
            },
            {"source": "hot:write", "target": "test:cache", "relation": "covered_by"},
            {"source": "hot:mutate", "target": "test:cache", "relation": "covered_by"},
        ],
    }
