# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate document search packet schema examples."""

from __future__ import annotations

import unittest

from .helpers import REPO_ROOT, schema_validator_for


class SemanticDocumentSearchPacketSchemaTests(unittest.TestCase):
    def test_document_search_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-search-packet.v1.schema.json"
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
            REPO_ROOT / "schemas" / "semantic-document-search-packet.v1.schema.json"
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
                base_fact | {
                    "kind": "frontMatter",
                    "sourceKind": "NodeValue::FrontMatter",
                },
                base_fact | {
                    "kind": "thematicBreak",
                    "sourceKind": "NodeValue::ThematicBreak",
                },
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

    def test_document_search_packet_accepts_toc_view(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-document-search-packet.v1.schema.json"
        )
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-document-search-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "md",
            "providerId": "orgize",
            "binary": "asp",
            "namespace": "agent.semantic-protocols.languages.md.orgize",
            "method": "search/toc",
            "projectRoot": ".",
            "view": "toc",
            "documentMode": "metadata",
            "query": "",
            "documentCount": 1,
            "factCount": 1,
            "owners": [
                {
                    "path": "README.md",
                    "role": "document",
                    "parserAuthority": "comrak",
                }
            ],
            "documentFacts": [
                {
                    "id": "heading:README.md:1:1",
                    "kind": "heading",
                    "sourceKind": "NodeValue::Heading",
                    "name": "Project",
                    "documentPath": "README.md",
                    "location": {"path": "README.md", "lineRange": "1:1"},
                    "parserAuthority": "comrak",
                    "queryKeys": ["Project", "heading"],
                    "attributes": {"level": "1", "title": "Project"},
                }
            ],
            "nextActions": [
                {
                    "kind": "query",
                    "target": "selector",
                    "command": "asp md query --selector README.md:1-1 --view metadata",
                }
            ],
            "notes": [
                {
                    "kind": "search-document",
                    "message": "Document TOC facts are heading metadata.",
                }
            ],
        }

        self.assertEqual([], list(validator.iter_errors(packet)))


if __name__ == "__main__":
    unittest.main()
