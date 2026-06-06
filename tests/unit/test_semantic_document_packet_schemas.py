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
                ["selector", "term", "metadata", "content"],
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
            "binary": "orgize",
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
                    "kind": "content",
                    "target": "selector",
                    "command": "orgize query --selector notes.org:2-4 --content",
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
            "binary": "orgize",
            "namespace": "agent.semantic-protocols.languages.md.orgize",
            "method": "query/document",
            "projectRoot": ".",
            "query": "README.md:1-1",
            "queryTerms": ["README.md:1-1"],
            "queryKind": "selector",
            "querySurface": "content",
            "documentMode": "content",
            "matchCount": 1,
            "matchLimit": 1,
            "matchesTruncated": False,
            "documentFacts": [],
            "contentBlocks": [
                {
                    "kind": "selector",
                    "documentPath": "README.md",
                    "location": {"path": "README.md", "lineRange": "1:1"},
                    "parserAuthority": "comrak",
                    "content": "# Project\n",
                }
            ],
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
            "binary": "orgize",
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
