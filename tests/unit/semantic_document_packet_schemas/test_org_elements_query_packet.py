"""Validate Org elements query packet schema examples."""

from __future__ import annotations

import unittest

from .helpers import REPO_ROOT, schema_validator_for


class SemanticOrgElementsQueryPacketSchemaTests(unittest.TestCase):
    def test_org_elements_query_packet_is_valid(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-org-elements-query-packet.v1.schema.json"
        )
        packet = {
            "schemaVersion": 1,
            "category": "element",
            "kind": "src-block",
            "affiliatedName": "task_runner",
            "summaryEquals": [{"key": "language", "value": "python"}],
            "propertyContains": [{"key": ":header-args", "needle": ":results"}],
            "relations": [{"type": "descendantOf", "ids": [1, 2]}],
            "predicate": {
                "all": [
                    {"kind": "src-block"},
                    {
                        "any": [
                            {"summary": {"key": "language", "equals": "python"}},
                            {"summary": {"key": "language", "equals": "rust"}},
                        ]
                    },
                    {"not": {"summary": {"key": "language", "equals": "shell"}}},
                ]
            },
            "limit": 5,
        }

        self.assertEqual([], list(validator.iter_errors(packet)))

    def test_org_elements_query_packet_rejects_invalid_version(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-org-elements-query-packet.v1.schema.json"
        )
        packet = {
            "schemaVersion": "1",
            "predicate": {"kind": "src-block"},
        }

        self.assertTrue(list(validator.iter_errors(packet)))

    def test_org_elements_query_packet_rejects_unknown_category(self) -> None:
        validator = schema_validator_for(
            REPO_ROOT / "schemas" / "semantic-org-elements-query-packet.v1.schema.json"
        )
        packet = {
            "schemaVersion": 1,
            "category": "drawer",
        }

        self.assertTrue(list(validator.iter_errors(packet)))


if __name__ == "__main__":
    unittest.main()
