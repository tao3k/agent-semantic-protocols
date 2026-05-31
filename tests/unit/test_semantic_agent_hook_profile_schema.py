"""Validate the shared agent hook profile registry schema."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_registry() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-hook-profile-registry",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.agent-hooks",
        "protocolVersion": "1",
        "projectRoot": ".",
        "profiles": [
            {
                "languageId": "typescript",
                "providerId": "ts-harness",
                "binary": "ts-harness",
                "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
                "sourceExtensions": [".ts", ".tsx"],
                "configFiles": ["package.json", "tsconfig.json"],
                "sourceRoots": ["src", "tests"],
                "ignoredPathPrefixes": ["node_modules", "dist", "coverage"],
                "policy": {
                    "blockDirectRead": True,
                    "blockBroadRawSearch": True,
                    "blockAgentSearchJson": True,
                    "requirePrimeBeforeEdit": True,
                },
                "commands": {
                    "prime": {"argv": ["ts-harness", "search", "prime", "."]},
                    "owner": {
                        "argv": ["ts-harness", "search", "owner", "{path}", "."]
                    },
                    "text": {
                        "argv": [
                            "ts-harness",
                            "search",
                            "text",
                            "{query}",
                            "owner",
                            "tests",
                            "--view",
                            "seeds",
                            ".",
                        ]
                    },
                    "ingest": {
                        "argv": [
                            "ts-harness",
                            "search",
                            "ingest",
                            "owner",
                            "tests",
                            "--view",
                            "seeds",
                            ".",
                        ],
                        "stdinMode": "pipe-candidates",
                    },
                    "checkChanged": {
                        "argv": ["ts-harness", "check", "--changed", "."]
                    },
                },
            }
        ],
    }


class SemanticAgentHookProfileSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-agent-hook-profile.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, registry: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(registry)]

    def test_minimal_profile_registry_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_registry()))

    def test_absolute_config_files_are_rejected(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        profile["configFiles"] = ["/etc/package.json"]
        registry["profiles"] = [profile]

        errors = self.validation_errors(registry)

        self.assertTrue(any("should not be valid" in message for message in errors))

    def test_commands_require_all_core_routes(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        del profile["commands"]["ingest"]
        registry["profiles"] = [profile]

        errors = self.validation_errors(registry)

        self.assertTrue(any("'ingest' is a required property" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
