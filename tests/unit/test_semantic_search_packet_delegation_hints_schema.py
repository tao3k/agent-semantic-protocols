"""Schema contract tests for semantic-search delegation hints."""

from __future__ import annotations

import unittest
from pathlib import Path

from unit.schema_validation import schema_validator_for
from unit.test_semantic_search_packet_schema import _semantic_search_minimal_packet


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


class SemanticSearchDelegationHintSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_accepts_advisory_search_subagent_contract(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["delegationHints"] = [
            {
                "profile": "asp-explorer",
                "decision": "advisory",
                "runtimeOwner": "agent-client",
                "modelClass": "cheap",
                "readOnly": True,
                "noCode": True,
                "targetActions": ["A1.fd-query", "A2.rg-query"],
                "maxCommands": 8,
                "maxTurns": 1,
                "reason": "query-selector-low-confidence",
                "receipt": {
                    "kind": "search-subagent",
                    "requiredFields": [
                        "role",
                        "evidence",
                        "missing",
                        "next",
                        "risk",
                    ],
                },
            }
        ]

        self.assertEqual([], self.validation_errors(packet))

    def test_rejects_provider_owned_routing(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["delegationHints"] = [
            {
                "profile": "asp-explorer",
                "decision": "required",
                "runtimeOwner": "provider",
                "readOnly": True,
                "noCode": True,
                "targetActions": ["fd-query"],
                "reason": "query-selector-low-confidence",
                "receipt": {
                    "kind": "search-subagent",
                    "requiredFields": ["role", "evidence", "missing", "next", "risk"],
                },
            }
        ]

        self.assertTrue(self.validation_errors(packet))


if __name__ == "__main__":
    unittest.main()
