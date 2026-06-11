"""Validate document query packet schema examples."""

from __future__ import annotations

import unittest

from .helpers import REPO_ROOT, schema_validator_for


class SemanticDocumentQueryPacketSchemaTests(unittest.TestCase):
    def test_document_query_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
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
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
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

    def test_document_query_packet_accepts_content_blocks(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "org",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.org.orgize",
            "method": "query/document",
            "projectRoot": ".",
            "query": "embedded",
            "queryTerms": ["embedded"],
            "queryKind": "term",
            "querySurface": "content",
            "documentMode": "content",
            "matchCount": 1,
            "matchLimit": 80,
            "matchesTruncated": False,
            "documentFacts": [],
            "contentBlocks": [
                {
                    "kind": "element",
                    "documentPath": "notes.org",
                    "location": {"path": "notes.org", "lineRange": "3:3"},
                    "parserAuthority": "orgize",
                    "contentKind": "documentation-metadata",
                    "criticality": "metadata",
                    "sourceFingerprint": "sha256:notes-org-section-3",
                    "compaction": {
                        "mode": "org-metadata-outline",
                        "lossiness": "aggressive",
                        "trustLevel": "metadata-backed",
                        "sourceOfTruth": "document-parser-facts",
                        "validFor": ["discovery", "routing"],
                        "notValidFor": ["quoting", "normative-proof"],
                        "preserved": ["headlines", "tags", "links"],
                        "omitted": ["body-paragraphs", "result-blocks"],
                    },
                    "content": "Document providers stay embedded inside ASP.",
                }
            ],
            "truncated": False,
        }

        self.assertEqual([], list(validator.iter_errors(packet)))


if __name__ == "__main__":
    unittest.main()
