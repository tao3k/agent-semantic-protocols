# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate document query packet schema examples."""

from __future__ import annotations

import unittest

from .helpers import REPO_ROOT, schema_validator_for


def packet_evidence() -> dict[str, object]:
    return {
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": "a" * 64,
            "sourceKind": "filesystem",
            "leafCount": 1,
            "providerDigest": "orgize-parser-v1",
        },
        "resolutionEvidence": {
            "schemaId": "agent.semantic-protocols.document-resolution-evidence",
            "snapshotRoot": "a" * 64,
            "authority": "live-parser",
            "state": "live-hit",
        },
        "itemDigest": "blake3:" + "b" * 64,
        "executionCommandDigest": "sha256:" + "c" * 64,
    }


class SemanticDocumentQueryPacketSchemaTests(unittest.TestCase):
    def test_document_query_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "md",
            "providerId": "asp-md",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.md.asp-md",
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
                    "structuralSelector": "md://README.md#heading/Project",
                    "location": {"path": "README.md", "lineRange": "1:1"},
                    "parserAuthority": "comrak",
                    "queryKeys": ["heading", "Project"],
                    "attributes": {"title": "Project", "level": "1"},
                    "textSnippet": "Project",
                }
            ],
            "contentBlocks": [],
            "truncated": False,
            **packet_evidence(),
        }

        self.assertEqual([], list(validator.iter_errors(packet)))

    def test_document_packet_rejects_source_language(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "rust",
            "providerId": "asp-rust",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
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
            **packet_evidence(),
        }

        self.assertTrue(list(validator.iter_errors(packet)))

    def test_document_query_packet_accepts_content_blocks(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-query-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-query-packet",
            "schemaVersion": "1",
            "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "org",
            "providerId": "asp-org",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.org.asp-org",
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
                    "structuralSelector": "org://notes.org#paragraph/3",
                    "location": {"path": "notes.org", "lineRange": "3:3"},
                    "parserAuthority": "orgize",
                    "content": "Document providers stay embedded inside ASP.",
                    "itemDigest": "blake3:" + "d" * 64,
                }
            ],
            "truncated": False,
            **packet_evidence(),
        }

        self.assertEqual([], list(validator.iter_errors(packet)))


if __name__ == "__main__":
    unittest.main()
