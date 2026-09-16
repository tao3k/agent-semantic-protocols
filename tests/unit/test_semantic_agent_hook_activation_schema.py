# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the agent hook activation schema consumed by provider repair."""

import unittest
from pathlib import Path
from typing import Any

class SemanticAgentHookActivationSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "hook-activation.v2.schema.json"
        )
        from unit.schema_validation import schema_validator_for

        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, activation: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(activation)]

    def valid_activation(self) -> dict[str, Any]:
        return {
            "schemaId": "agent.semantic-protocols.hook.activation",
            "schemaVersion": "2",
            "schemaAuthority": "https://tao3k.github.io/agent-semantic-protocols/schemas/",
            "protocolId": "agent.semantic-protocols.hook",
            "protocolVersion": "1",
            "projectRoot": ".",
            "generatedBy": {"runtime": "asp", "version": "0.1.0"},
            "rankers": [],
            "providers": [
                {
                    "manifestId": "agent.semantic-protocols.providers.rust.asp-rust",
                    "manifestDigest": "sha256:" + "a" * 64,
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "searchCapabilities": {
                        "ownerItems": True,
                        "semanticFacts": True,
                        "dependencyTopology": True,
                        "dependencyTopologyMetadata": True,
                    },
                    "queryPackDescriptor": {
                        "descriptorId": "rust.query-pack",
                        "descriptorVersion": "1",
                        "languageId": "rust",
                        "recipes": [],
                    },
                    "semanticRegistryDigest": "sha256:" + "b" * 64,
                    "routes": {
                        "playbook": {"argv": ["search", "playbook"]},
                        "checkChanged": {"argv": ["check-changed"]},
                    },
                    "coverage": {
                        "packageRoots": ["."],
                        "configFiles": ["Cargo.toml"],
                        "sourceExtensions": [".rs"],
                    },
                }
            ],
        }

    def test_activation_generated_by_asp_is_valid(self):
        self.assertEqual([], self.validation_errors(self.valid_activation()))

    def test_activation_rejects_retired_schema_version(self):
        activation = self.valid_activation()
        activation["schemaVersion"] = "1"

        self.assertTrue(
            any("'2' was expected" in error for error in self.validation_errors(activation))
        )


if __name__ == "__main__":
    unittest.main()
