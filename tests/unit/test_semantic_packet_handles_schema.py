"""Validate semantic handle embedding in search and query packets."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_handle() -> dict[str, object]:
    return {
        "id": "TS-SCHEMA-FIXTURE:semantic-search-packet:query-set",
        "kind": "schema-fixture",
        "source": "schema",
        "title": "semantic search packet query-set fixture",
        "aliases": ["query-set-fixture", "minimal-packet"],
        "labels": ["schema", "query-set"],
        "ownerPath": "tests/unit/test_semantic_search_packet_query_set_schema.py",
        "implementationOwnerPath": "schemas/semantic-search-packet.v1.schema.json",
        "testPaths": ["tests/unit/test_semantic_search_packet_query_set_schema.py"],
        "queryTerms": ["minimal_packet", "querySet"],
        "fields": {"schemaId": "agent.semantic-protocols.semantic-search-packet"},
    }


def search_packet_with_handle() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/policy",
        "projectRoot": ".",
        "view": "policy",
        "renderMode": "hits",
        "header": {"kind": "search-policy", "fields": {}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "hits": [],
        "findings": [],
        "nextActions": [],
        "notes": [],
        "searchSynthesis": {
            "algorithm": "policy-handle-catalog",
            "scope": "policy",
            "summary": "resolved provider-owned policy handles",
        },
        "semanticHandles": [semantic_handle()],
    }


def query_packet_with_handle() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "query/semantic-handles",
        "projectRoot": ".",
        "ownerPath": "tests/unit/test_semantic_search_packet_query_set_schema.py",
        "query": "minimal_packet",
        "queryTerms": ["minimal_packet"],
        "matchMode": "exact",
        "outputMode": "names",
        "queryCoverage": [
            {
                "value": "minimal_packet",
                "status": "hit",
                "match": "exact",
                "matchCount": 1,
            }
        ],
        "semanticHandles": [semantic_handle()],
        "matches": [],
        "truncated": False,
        "notes": [],
    }


class SemanticPacketHandlesSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_dir = _REPO_ROOT / "schemas"
        with (schema_dir / "semantic-search-packet.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            self.search_schema = json.load(handle)
        with (schema_dir / "semantic-query-packet.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            self.query_schema = json.load(handle)
        with (schema_dir / "semantic-handle.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            handle_schema = json.load(handle)
        registry = Registry().with_resources(
            [
                (self.search_schema["$id"], Resource.from_contents(self.search_schema)),
                (self.query_schema["$id"], Resource.from_contents(self.query_schema)),
                (handle_schema["$id"], Resource.from_contents(handle_schema)),
            ]
        )
        self.search_validator = Draft202012Validator(
            self.search_schema, registry=registry
        )
        self.query_validator = Draft202012Validator(self.query_schema, registry=registry)

    def search_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.search_validator.iter_errors(packet)]

    def query_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.query_validator.iter_errors(packet)]

    def test_search_packet_accepts_semantic_handles(self) -> None:
        self.assertEqual([], self.search_errors(search_packet_with_handle()))

    def test_query_packet_accepts_semantic_handles_without_code(self) -> None:
        self.assertEqual([], self.query_errors(query_packet_with_handle()))


if __name__ == "__main__":
    unittest.main()
