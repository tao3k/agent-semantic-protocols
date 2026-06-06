"""Validate document-language search/query packet schemas."""

from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path
from typing import Callable


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_VALIDATION_PATH = _REPO_ROOT / "tests" / "unit" / "schema_validation.py"
_SCHEMA_VALIDATION_SPEC = importlib.util.spec_from_file_location(
    "schema_validation", _SCHEMA_VALIDATION_PATH
)
assert _SCHEMA_VALIDATION_SPEC is not None
assert _SCHEMA_VALIDATION_SPEC.loader is not None
_SCHEMA_VALIDATION_MODULE = importlib.util.module_from_spec(_SCHEMA_VALIDATION_SPEC)
_SCHEMA_VALIDATION_SPEC.loader.exec_module(_SCHEMA_VALIDATION_MODULE)

schema_validator_for: Callable[[Path], object] = (
    _SCHEMA_VALIDATION_MODULE.schema_validator_for
)


class SemanticDocumentPacketSchemaTests(unittest.TestCase):
    def test_provider_registry_advertises_document_packet_schemas(self) -> None:
        validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "semantic-language-registry.v1.schema.json"
        )
        registry = json.loads(
            (
                _REPO_ROOT
                / "schemas"
                / "semantic-language-registry.providers.v1.json"
            ).read_text()
        )

        self.assertEqual([], list(validator.iter_errors(registry)))
        languages = {item["languageId"]: item for item in registry["languages"]}
        self.assertIn("org", languages)
        self.assertIn("md", languages)
        for language_id in ["org", "md"]:
            self.assertNotIn("search/owner", languages[language_id]["methods"])
            packet_schemas = {
                packet_schema
                for descriptor in languages[language_id]["methodDescriptors"]
                for packet_schema in descriptor.get("packetSchemas", [])
            }
            self.assertIn("semantic-document-search-packet.v1", packet_schemas)
            self.assertIn("semantic-document-query-packet.v1", packet_schemas)
            document_query = next(
                descriptor
                for descriptor in languages[language_id]["methodDescriptors"]
                if descriptor["method"] == "query/document"
            )
            self.assertEqual(
                ["selector", "term", "metadata"],
                document_query["queryInputForms"],
            )

    def test_document_search_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "semantic-document-search-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-search-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "org",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.org.orgize",
            "method": "search/prime",
            "projectRoot": ".",
            "view": "prime",
            "documentMode": "metadata",
            "query": "",
            "documentCount": 1,
            "factCount": 1,
            "owners": [
                {
                    "path": "notes.org",
                    "role": "document",
                    "parserAuthority": "orgize",
                }
            ],
            "documentFacts": [
                {
                    "id": "property:notes.org:2:4",
                    "kind": "property",
                    "sourceKind": "PropertyDrawer",
                    "name": "CUSTOM_ID",
                    "documentPath": "notes.org",
                    "location": {"path": "notes.org", "lineRange": "2:4"},
                    "parserAuthority": "orgize",
                    "queryKeys": ["CUSTOM_ID", "property"],
                    "attributes": {"key": "CUSTOM_ID", "value": "task-1"},
                }
            ],
            "nextActions": [
                {
                    "kind": "query",
                    "target": "selector",
                    "command": "asp org query --selector notes.org:2-4 --view metadata",
                }
            ],
            "notes": [
                {
                    "kind": "search-document",
                    "message": "Document facts are metadata.",
                }
            ],
        }

        self.assertEqual([], list(validator.iter_errors(packet)))

    def test_document_search_packet_accepts_element_map_kinds(self) -> None:
        validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "semantic-document-search-packet.v1.schema.json"
        )
        base_fact = {
            "id": "task:README.md:4:4",
            "name": "Write tests",
            "documentPath": "README.md",
            "location": {"path": "README.md", "lineRange": "4:4"},
            "parserAuthority": "comrak",
            "queryKeys": ["task", "Write tests"],
            "attributes": {},
        }
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-search-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "md",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.md.orgize",
            "method": "search/prime",
            "projectRoot": ".",
            "view": "prime",
            "documentMode": "metadata",
            "query": "",
            "documentCount": 1,
            "factCount": 5,
            "owners": [
                {
                    "path": "README.md",
                    "role": "document",
                    "parserAuthority": "comrak",
                }
            ],
            "documentFacts": [
                base_fact | {"kind": "task", "sourceKind": "NodeValue::TaskItem"},
                base_fact | {"kind": "list", "sourceKind": "NodeValue::List"},
                base_fact | {"kind": "image", "sourceKind": "NodeValue::Image"},
                base_fact | {"kind": "frontMatter", "sourceKind": "NodeValue::FrontMatter"},
                base_fact | {"kind": "thematicBreak", "sourceKind": "NodeValue::ThematicBreak"},
            ],
            "nextActions": [
                {
                    "kind": "query",
                    "target": "selector",
                    "command": "asp md query --selector README.md:4-4 --view metadata",
                }
            ],
            "notes": [
                {
                    "kind": "search-document",
                    "message": "Document facts are metadata.",
                }
            ],
        }

        self.assertEqual([], list(validator.iter_errors(packet)))

    def test_document_query_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "md",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.md.orgize",
            "method": "query/document",
            "projectRoot": ".",
            "query": "README.md:1-1",
            "queryTerms": ["README.md:1-1"],
            "queryKind": "selector",
            "querySurface": "metadata",
            "documentMode": "metadata",
            "matchCount": 1,
            "matchLimit": 80,
            "matchesTruncated": False,
            "documentFacts": [
                {
                    "id": "heading:README.md:1:1",
                    "kind": "heading",
                    "sourceKind": "NodeValue::Heading",
                    "name": "Project",
                    "documentPath": "README.md",
                    "location": {"path": "README.md", "lineRange": "1:1"},
                    "parserAuthority": "comrak",
                    "queryKeys": ["heading", "Project"],
                    "attributes": {"title": "Project", "level": "1"},
                }
            ],
            "contentBlocks": [],
            "truncated": False,
        }

        self.assertEqual([], list(validator.iter_errors(packet)))

    def test_document_packet_rejects_source_language(self) -> None:
        validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "rust",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.rust.orgize",
            "method": "query/document",
            "projectRoot": ".",
            "query": "*",
            "queryTerms": ["*"],
            "queryKind": "term",
            "querySurface": "metadata",
            "documentMode": "metadata",
            "matchCount": 0,
            "matchLimit": 1,
            "matchesTruncated": False,
            "documentFacts": [],
            "contentBlocks": [],
            "truncated": False,
        }

        self.assertTrue(list(validator.iter_errors(packet)))


if __name__ == "__main__":
    unittest.main()
