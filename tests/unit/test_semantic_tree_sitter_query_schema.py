"""Semantic tree-sitter query packet schema tests."""

import copy
import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def semantic_tree_sitter_query_packet() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "method": "query",
        "projectRoot": ".",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "sourceAuthority": "native-parser-adapter",
        "adapterMode": "native-projection",
        "compatibilityLevel": "native-only",
        "query": {
            "input": "calls",
            "inputForm": "catalog-id",
            "dialect": "tree-sitter-query",
            "catalogId": "calls",
            "catalogPath": "tree-sitter/tree-sitter-rust/queries/calls.scm",
            "grammarProfilePath": "tree-sitter/tree-sitter-rust/grammar-profile.json",
            "compiledSource": "(call_expression function: (_) @call.target) @call.expression",
            "fields": {
                "captures": ["call.expression", "call.target"],
                "catalogCanonical": True,
                "catalogEmbedded": True,
                "compilerBoundary": "asp-tree-sitter-runtime",
                "providerRuntimeCompiled": False,
            },
        },
        "matches": [],
        "truncated": False,
        "cache": {
            "cacheStatus": "miss",
            "requestFingerprint": "semantic-tree-sitter-query.v1:rust:tree-sitter-rust:calls:catalog:profile",
            "generationId": "rust-tree-sitter-query:calls:2026-06-04.v1",
            "artifactId": "semantic-tree-sitter-query/calls.json",
            "artifactKind": "semantic-tree-sitter-query",
            "catalogFingerprint": "rust-default:1111111111111111",
            "grammarProfileFingerprint": "rust-default:2222222222222222",
            "rawSourceStored": False,
        },
    }


class SemanticTreeSitterQuerySchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "semantic-tree-sitter-query.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_binary_embedded_catalog_query_packet_is_valid(self) -> None:
        packet = semantic_tree_sitter_query_packet()

        assert self.validation_errors(packet) == []

    def test_query_object_accepts_grammar_profile_path(self) -> None:
        packet = semantic_tree_sitter_query_packet()

        assert packet["query"]["grammarProfilePath"]
        assert self.validation_errors(packet) == []

    def test_accepts_native_projected_matches_and_capture_enrichment(self) -> None:
        packet = copy.deepcopy(semantic_tree_sitter_query_packet())
        native_ref = "rust:item:src/lib.rs:1:3:exposed"
        packet["nativeFactRefs"] = [native_ref]
        packet["matches"] = [
            {
                "id": "match.1",
                "patternIndex": 0,
                "range": {"path": "src/lib.rs", "lineRange": "1:3"},
                "captures": [
                    {
                        "id": "capture.1",
                        "name": "function.name",
                        "nodeType": "function_item",
                        "field": "name",
                        "named": True,
                        "range": {"path": "src/lib.rs", "lineRange": "1:1"},
                        "nativeFactRefs": [native_ref],
                        "semanticHandleRefs": ["symbol:exposed"],
                        "fields": {
                            "symbol": "exposed",
                            "read": "src/lib.rs:1:1",
                            "itemRead": "src/lib.rs:1:3",
                            "sourceAuthority": "native-parser",
                            "nativeNodeType": "function_item",
                            "semanticKind": "function",
                        },
                    }
                ],
                "nativeFactRefs": [native_ref],
                "semanticHandleRefs": ["symbol:exposed"],
                "fields": {
                    "symbol": "exposed",
                    "read": "src/lib.rs:1:1",
                    "itemRead": "src/lib.rs:1:3",
                    "nodeType": "function_item",
                    "captureCount": 1,
                },
            }
        ]

        assert self.validation_errors(packet) == []

    def test_rejects_unknown_query_object_property(self) -> None:
        packet = semantic_tree_sitter_query_packet()
        packet["query"]["sourceDelivery"] = "provider-binary-embedded"

        assert any(
            "Additional properties are not allowed" in error
            and "sourceDelivery" in error
            for error in self.validation_errors(packet)
        )

    def test_catalog_embedded_is_a_scalar_field_not_a_new_packet_surface(self) -> None:
        packet = copy.deepcopy(semantic_tree_sitter_query_packet())
        packet["query"]["fields"]["catalogEmbedded"] = True

        assert self.validation_errors(packet) == []


if __name__ == "__main__":
    unittest.main()
