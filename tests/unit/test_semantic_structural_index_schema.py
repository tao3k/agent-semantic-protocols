"""Validate structural feeds with non-empty native syntax facts."""

from __future__ import annotations

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
            "fileHashes": [{"path": "src/widget.cc", "sha256": "0" * 64}],
            "owners": [],
            "symbols": [],
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
                    "location": {"path": "src/widget.cc", "lineRange": "2:2"},
                    "visibility": "public",
                    "queryKeys": ["value", "Widget::value"],
                    "fields": {"symbolId": "c:@S@Widget@F@value#1"},
                }
            ],
        }

        errors = list(schema_validator_for(_SCHEMA).iter_errors(packet))
        self.assertEqual([], errors, "\n".join(error.message for error in errors))


if __name__ == "__main__":
    unittest.main()
