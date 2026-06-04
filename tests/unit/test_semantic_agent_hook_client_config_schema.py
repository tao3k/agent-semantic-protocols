from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


class SemanticAgentHookClientConfigSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT
            / "schemas"
            / "semantic-agent-hook-client-config.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, config: dict) -> list[str]:
        return [error.message for error in self.validator.iter_errors(config)]

    def test_full_client_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.hook.client-config",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.hook",
            "protocolVersion": "1",
            "rules": [
                {
                    "id": "deny-rust-rg",
                    "enabled": True,
                    "priority": 100,
                    "decision": "deny",
                    "reasonKind": "raw-broad-search",
                    "languageIds": ["rust"],
                    "event": "pre-tool",
                    "platform": "codex",
                    "message": "custom config deny",
                    "match": {
                        "tool": "Bash",
                        "commandAny": ["rg"],
                        "pathGlobAny": ["**/*.rs"],
                    },
                    "routes": [
                        {
                            "providerId": "rs-harness",
                            "languageId": "rust",
                            "binary": "rs-harness",
                            "kind": "ingest",
                            "argv": [
                                "rs-harness",
                                "search",
                                "ingest",
                                "items",
                                "tests",
                                "--view",
                                "seeds",
                                ".",
                            ],
                            "stdinMode": "pipe-candidates",
                        }
                    ],
                }
            ],
        }

        self.assertEqual(self.validation_errors(config), [])

    def test_minimal_client_config_is_valid(self) -> None:
        self.assertEqual(
            self.validation_errors({"rules": [{"id": "block", "decision": "block"}]}),
            [],
        )

    def test_client_config_rejects_wrong_schema_id(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.wrong",
                "rules": [{"id": "block", "decision": "block"}],
            }
        )
        self.assertTrue(any("was expected" in error for error in errors))

    def test_client_config_rejects_unknown_rule_fields(self) -> None:
        errors = self.validation_errors(
            {
                "rules": [
                    {
                        "id": "block",
                        "decision": "block",
                        "script": "return deny",
                    }
                ]
            }
        )
        self.assertTrue(any("Additional properties" in error for error in errors))

    def test_client_config_rejects_duplicate_language_ids(self) -> None:
        errors = self.validation_errors(
            {
                "rules": [
                    {
                        "id": "block",
                        "decision": "block",
                        "languageIds": ["rust", "rust"],
                    }
                ]
            }
        )
        self.assertTrue(any("non-unique elements" in error for error in errors))

    def test_client_config_rejects_empty_route_argv(self) -> None:
        errors = self.validation_errors(
            {
                "rules": [
                    {
                        "id": "block",
                        "decision": "block",
                        "routes": [
                            {
                                "providerId": "rs-harness",
                                "kind": "query",
                                "argv": [],
                            }
                        ],
                    }
                ]
            }
        )
        self.assertTrue(any("should be non-empty" in error for error in errors))

    def test_client_config_rejects_invalid_route_binary(self) -> None:
        errors = self.validation_errors(
            {
                "rules": [
                    {
                        "id": "block",
                        "decision": "block",
                        "routes": [
                            {
                                "providerId": "rs-harness",
                                "binary": "../rs-harness",
                                "kind": "query",
                                "argv": ["rs-harness"],
                            }
                        ],
                    }
                ]
            }
        )
        self.assertTrue(any("does not match" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
