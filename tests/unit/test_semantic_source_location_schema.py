"""Validate shared semantic source-location schema references."""

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_SCHEMA_DIR = Path(__file__).resolve().parents[2] / "schemas"


def _load_schema(name: str) -> dict[str, object]:
    with (_SCHEMA_DIR / name).open("r", encoding="utf-8") as handle:
        return json.load(handle)


class SemanticSourceLocationSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.schema = _load_schema("semantic-source-location.v1.schema.json")
        self.validator = Draft202012Validator(self.schema)

    def test_valid_source_location_bundle(self) -> None:
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-source-location",
            "schemaVersion": "1",
            "path": "src/demo.py",
            "lineRange": "3:8",
            "location": {"path": "src/demo.py", "lineRange": "3:8"},
            "sourceLocator": "src/demo.py:3-8",
            "sourceSpanLocator": "src/demo.py:3:8",
        }

        self.assertEqual([], list(self.validator.iter_errors(packet)))

    def test_project_path_rejects_rank_prefix_and_absolute_path(self) -> None:
        validator = Draft202012Validator(self.schema["$defs"]["projectPath"])

        for path in ("1:src/demo.py", "/tmp/demo.py", "../demo.py"):
            with self.subTest(path=path):
                self.assertNotEqual([], list(validator.iter_errors(path)))

    def test_source_span_locator_requires_explicit_range(self) -> None:
        validator = Draft202012Validator(self.schema["$defs"]["sourceSpanLocator"])

        self.assertEqual([], list(validator.iter_errors("src/demo.py:3:8")))
        self.assertNotEqual([], list(validator.iter_errors("src/demo.py:3")))
        self.assertNotEqual([], list(validator.iter_errors("src/demo.py:3-8")))

    def test_packet_and_tree_sitter_schemas_reference_source_location_base(
        self,
    ) -> None:
        expected_refs = {
            "semantic-query-packet.v1.schema.json": {
                "projectPath": "projectPath",
                "location": "location",
                "sourceLocator": "sourceSpanLocator",
            },
            "semantic-search-packet.v1.schema.json": {
                "projectPath": "projectPath",
                "location": "location",
            },
            "semantic-read-packet.v1.schema.json": {
                "projectPath": "projectPath",
                "lineRange": "lineRange",
                "location": "location",
                "sourceLocator": "sourceLocator",
            },
            "semantic-native-syntax-fact-index.v1.schema.json": {
                "projectPath": "projectPath",
                "location": "location",
            },
            "semantic-tree-sitter-query.v1.schema.json": {
                "projectPath": "projectPath",
                "lineRange": "lineRange",
                "location": "location",
                "sourceLocator": "sourceLocator",
                "sourceSpanLocator": "sourceSpanLocator",
            },
            "semantic-tree-sitter-grammar-profile.v1.schema.json": {
                "projectPath": "projectPath",
            },
            "semantic-tree-sitter-provenance.v1.schema.json": {
                "projectPath": "projectPath",
                "lineRange": "lineRange",
                "location": "location",
            },
        }

        for schema_name, refs in expected_refs.items():
            defs = _load_schema(schema_name)["$defs"]
            for local_name, source_location_name in refs.items():
                with self.subTest(schema=schema_name, local_name=local_name):
                    self.assertEqual(
                        defs[local_name]["$ref"],
                        "semantic-source-location.v1.schema.json"
                        f"#/$defs/{source_location_name}",
                    )
