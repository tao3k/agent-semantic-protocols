"""Validate shared tree-sitter provenance schema references."""

from __future__ import annotations

import json
import unittest
from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_DIR = _REPO_ROOT / "schemas"


def _load_schema(name: str) -> dict[str, object]:
    with (_SCHEMA_DIR / name).open("r", encoding="utf-8") as handle:
        return json.load(handle)


class SemanticTreeSitterProvenanceSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        from unit.schema_validation import schema_validator_for

        self.schema = _load_schema("semantic-tree-sitter-provenance.v1.schema.json")
        self.validator = schema_validator_for(
            _SCHEMA_DIR / "semantic-tree-sitter-provenance.v1.schema.json"
        )

    def test_valid_tree_sitter_provenance_bundle(self) -> None:
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-tree-sitter-provenance",
            "schemaVersion": "1",
            "syntaxQueryRef": "semantic-tree-sitter-query/typescript-owner-items:src:alpha",
            "syntaxMatchRefs": ["match:1"],
            "syntaxCaptureRefs": ["capture:1"],
            "syntaxAnchor": {
                "nodeType": "function_declaration",
                "field": "name",
                "capture": "function.name",
                "location": {"path": "src/demo.ts", "lineRange": "1:3"},
            },
        }

        self.assertEqual([], list(self.validator.iter_errors(packet)))

    def test_tree_sitter_provenance_requires_schema_identity(self) -> None:
        errors = list(self.validator.iter_errors({"schemaVersion": "1"}))

        self.assertIn("'schemaId' is a required property", _messages(errors))

    def test_shared_base_does_not_define_packet_envelope(self) -> None:
        provenance = self.schema["$defs"]["treeSitterProvenance"]

        self.assertEqual("object", provenance["type"])
        self.assertNotIn("method", provenance["properties"])
        self.assertNotIn("view", provenance["properties"])
        self.assertNotIn("matches", provenance["properties"])
        self.assertNotIn("sourceWindows", provenance["properties"])

    def test_packet_fields_reference_tree_sitter_provenance_base(self) -> None:
        for schema_name in (
            "semantic-search-packet.v1.schema.json",
            "semantic-query-packet.v1.schema.json",
            "semantic-read-packet.v1.schema.json",
        ):
            with self.subTest(schema=schema_name):
                self._assert_packet_schema_uses_tree_sitter_provenance(
                    _load_schema(schema_name)
                )

    def _assert_packet_schema_uses_tree_sitter_provenance(
        self, schema: dict[str, object]
    ) -> None:
        props = schema["properties"]

        for key in (
            "syntaxQueryRef",
            "syntaxMatchRefs",
            "syntaxCaptureRefs",
            "syntaxAnchor",
        ):
            self.assertEqual(
                props[key]["$ref"],
                f"semantic-tree-sitter-provenance.v1.schema.json#/$defs/{key}",
            )
        self.assertNotIn("syntaxAnchor", schema["$defs"])


def _messages(errors: list[object]) -> str:
    return "\n".join(error.message for error in errors)
