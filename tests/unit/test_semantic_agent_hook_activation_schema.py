"""Validate the agent hook activation schema consumed by provider repair."""

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


class SemanticAgentHookActivationSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "semantic-agent-hook-activation.v1.schema.json"
        )
        with open(schema_path, "r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, activation: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(activation)]

    def valid_activation(self) -> dict[str, Any]:
        return {
            "schemaId": "agent.semantic-protocols.hook.activation",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.hook",
            "protocolVersion": "1",
            "projectRoot": ".",
            "generatedBy": {"runtime": "asp", "version": "0.1.0"},
            "providers": [
                {
                    "manifestId": "agent.semantic-protocols.providers.rust.rs-harness",
                    "manifestDigest": "sha256:" + "a" * 64,
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "binary": "rs-harness",
                    "providerCommandPrefix": [],
                    "coverage": {
                        "packageRoots": ["."],
                        "sourceRoots": ["src", "tests"],
                        "configFiles": ["Cargo.toml"],
                        "sourceExtensions": [".rs"],
                        "ignoredPathPrefixes": ["target"],
                    },
                }
            ],
        }

    def test_activation_generated_by_asp_is_valid(self):
        self.assertEqual([], self.validation_errors(self.valid_activation()))

    def test_activation_rejects_retired_hook_runtime_name(self):
        activation = self.valid_activation()
        activation["generatedBy"]["runtime"] = "agent-semantic-hook"

        self.assertTrue(
            any("'asp' was expected" in error for error in self.validation_errors(activation))
        )


if __name__ == "__main__":
    unittest.main()
