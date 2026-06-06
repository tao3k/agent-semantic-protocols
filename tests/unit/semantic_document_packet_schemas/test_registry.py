"""Validate document provider registry schema descriptors."""

from __future__ import annotations

import json
import unittest

from .helpers import REPO_ROOT, schema_validator_for


class SemanticDocumentRegistrySchemaTests(unittest.TestCase):
    def test_provider_registry_advertises_document_packet_schemas(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-language-registry.v1.schema.json"
        )
        registry = json.loads(
            (
                REPO_ROOT
                / "schemas"
                / "semantic-language-registry.providers.v1.json"
            ).read_text()
        )

        self.assertEqual([], list(validator.iter_errors(registry)))
        languages = {item["languageId"]: item for item in registry["languages"]}
        self.assertIn("org", languages)
        self.assertIn("md", languages)
        for language_id in ["org", "md"]:
            self.assertEqual("embedded", languages[language_id]["execution"])
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
                ["selector", "term", "kind", "field", "metadata", "content"],
                document_query["queryInputForms"],
            )


if __name__ == "__main__":
    unittest.main()
