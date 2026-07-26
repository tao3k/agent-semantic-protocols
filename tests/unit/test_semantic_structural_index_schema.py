"""Validate structural feeds with non-empty native syntax facts."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from tests.unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = _REPO_ROOT / "schemas" / "semantic-structural-index.v1.schema.json"


class SemanticStructuralIndexSchemaTests(unittest.TestCase):
    def test_non_empty_native_syntax_facts_resolve_local_schema_graph(self) -> None:
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-structural-index",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "generationId": "sha256:test",
            "languageId": "cpp",
            "providerId": "ccls-asp",
            "exportMethod": "index/structural",
            "projectRoot": ".",
            "rawSourceStored": False,
            "compileContexts": [
                {"translationUnit": "src/widget.cc", "digest": "1" * 64}
            ],
            "fileHashes": [
                {
                    "path": "src/widget.cc",
                    "sha256": "0" * 64,
                    "compileContextDigest": "1" * 64,
                }
            ],
            "owners": [],
            "symbols": [
                {
                    "ownerPath": "src/widget.cc",
                    "name": "value",
                    "qualifiedName": "Widget::value",
                    "kind": "method",
                    "structuralSelector": (
                        "cpp://src/widget.cc#clang-usr:c:@S@Widget@F@value:method"
                    ),
                    "queryKeys": ["value", "Widget::value"],
                }
            ],
            "occurrences": [
                {
                    "id": "clang-occurrence:call:value@src/widget.cc:40:45",
                    "ownerPath": "src/widget.cc",
                    "name": "value",
                    "kind": "call",
                    "sourceLocator": "src/widget.cc:4:4",
                    "targetSymbolId": "c:@S@Widget@F@value#1",
                    "queryKeys": ["value", "Widget::value"],
                }
            ],
            "relations": [
                {
                    "id": "clang-occurrence:override:value@src/widget.cc:20:25",
                    "ownerPath": "src/widget.cc",
                    "name": "value",
                    "kind": "override",
                    "sourceLocator": "src/widget.cc:2:2",
                    "targetSymbolId": "c:@S@Entity@F@value#1",
                    "queryKeys": ["value", "Widget::value"],
                }
            ],
            "dependencyUsages": [],
            "syntaxFacts": [
                {
                    "id": "clang:method:c:@S@Widget@F@value#1@src/widget.cc:2:2",
                    "kind": "method",
                    "source": "native-parser",
                    "languageKind": "method",
                    "name": "value",
                    "qualifiedName": "Widget::value",
                    "ownerPath": "src/widget.cc",
                    "location": {
                        "path": "src/widget.cc",
                        "lineRange": "2:2",
                        "structuralSelector": (
                            "cpp://src/widget.cc#clang-usr:c:@S@Widget@F@value:method"
                        ),
                        "displayLineRange": "2:2",
                        "sourceLocatorHint": "src/widget.cc:2:2",
                    },
                    "visibility": "public",
                    "queryKeys": ["value", "Widget::value"],
                    "fields": {"symbolId": "c:@S@Widget@F@value#1"},
                }
            ],
        }

        errors = list(schema_validator_for(_SCHEMA).iter_errors(packet))
        self.assertEqual([], errors, "\n".join(error.message for error in errors))

        missing_target = copy.deepcopy(packet)
        del missing_target["relations"][0]["targetSymbolId"]
        target_errors = list(
            schema_validator_for(_SCHEMA).iter_errors(missing_target)
        )
        self.assertTrue(
            any(
                "'targetSymbolId' is a required property" in error.message
                for error in target_errors
            )
        )


if __name__ == "__main__":
    unittest.main()
