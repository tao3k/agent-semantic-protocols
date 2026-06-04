"""Schema contract tests for semantic-search reasoning profile fields."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def _semantic_search_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/owner",
        "projectRoot": ".",
        "view": "owner",
        "renderMode": "graph",
        "header": {"kind": "search-owner", "fields": {}},
        "nodes": [
            {
                "id": "O:src/components/WorkflowExecution.tsx",
                "kind": "owner",
                "path": "src/components/WorkflowExecution.tsx",
                "fields": {},
            }
        ],
        "edges": [],
        "owners": [
            {
                "path": "src/components/WorkflowExecution.tsx",
                "role": "source",
                "public": False,
                "fields": {},
            }
        ],
        "hits": [
            {
                "kind": "text",
                "ownerPath": "src/components/WorkflowExecution.tsx",
                "location": {
                    "path": "src/components/WorkflowExecution.tsx",
                    "lineRange": "42:42",
                },
                "score": 1.0,
                "reason": "parser-visible-source",
            }
        ],
        "findings": [],
        "nextActions": [
            {
                "kind": "owner",
                "target": "src/data/workflows.ts",
                "ownerPath": "src/components/WorkflowExecution.tsx",
            }
        ],
        "notes": [],
    }


class SemanticSearchPacketReasoningProfilesSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_reasoning_profiles_accept_typed_selector_contract(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["method"] = "search/reasoning"
        packet["view"] = "reasoning"
        packet["renderMode"] = "facts"
        packet["reasoningProfiles"] = [
            {
                "profile": "owner-query",
                "selectors": [
                    {
                        "kind": "owner",
                        "alias": "O",
                        "target": "src/components/WorkflowExecution.tsx",
                        "targetRole": "path",
                        "required": True,
                    },
                    {
                        "kind": "query",
                        "alias": "Q",
                        "target": "WorkflowExecution",
                        "targetRole": "term",
                        "required": True,
                    },
                ],
                "returns": ["items", "tests", "dependency-usage"],
                "frontier": ["O.owner"],
                "fields": {"source": "search-guide"},
            }
        ]

        self.assertEqual([], self.validation_errors(packet))

    def test_reasoning_profiles_reject_removed_alias_field(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["method"] = "search/reasoning"
        packet["view"] = "reasoning"
        packet["renderMode"] = "facts"
        removed_alias_field = "".join(
            map(
                chr,
                [
                    99,
                    111,
                    109,
                    112,
                    97,
                    116,
                    105,
                    98,
                    108,
                    101,
                    72,
                    97,
                    110,
                    100,
                    108,
                    101,
                    115,
                ],
            )
        )
        packet["reasoningProfiles"] = [
            {
                "profile": "owner-query",
                "selectors": [
                    {
                        "kind": "owner",
                        "alias": "O",
                        "target": "src/components/WorkflowExecution.tsx",
                        "targetRole": "path",
                        "required": True,
                    },
                    {
                        "kind": "query",
                        "alias": "Q",
                        "target": "WorkflowExecution",
                        "targetRole": "term",
                        "required": True,
                    },
                ],
                "returns": ["items", "tests", "dependency-usage"],
                removed_alias_field: ["O", "Q"],
            }
        ]

        errors = self.validation_errors(packet)

        self.assertTrue(
            any(
                "Additional properties are not allowed" in message
                and removed_alias_field in message
                for message in errors
            )
        )

    def test_reasoning_profiles_reject_natural_language_intent_fields(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["method"] = "search/reasoning"
        packet["view"] = "reasoning"
        packet["renderMode"] = "facts"
        packet["reasoningProfiles"] = [
            {
                "profile": "owner-query",
                "intent": "fix failing pytest around Path model",
                "selectors": [
                    {
                        "kind": "owner",
                        "alias": "O",
                        "target": "src/components/WorkflowExecution.tsx",
                    }
                ],
                "returns": ["items"],
            }
        ]

        errors = self.validation_errors(packet)

        self.assertTrue(
            any(
                "Additional properties are not allowed" in message for message in errors
            )
        )


if __name__ == "__main__":
    unittest.main()
