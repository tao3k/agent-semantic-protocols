# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate runtime profile schema facts used by agent provider repair."""

import json
import unittest
from pathlib import Path
from typing import Any

class SemanticAgentRuntimeProfilesSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "semantic-agent-runtime-profiles.v1.schema.json"
        )
        from unit.schema_validation import schema_validator_for

        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, profiles: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(profiles)]

    def valid_profiles(self) -> dict[str, Any]:
        return {
            "schemaId": "agent.semantic-protocols.runtime.profiles",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.runtime",
            "protocolVersion": "1",
            "projectRoot": "/tmp/project",
            "runtimeHome": "/tmp/project/.cache/agent-semantic-protocol/runtime",
            "generatedBy": {"runtime": "asp", "version": "0.1.0"},
            "providers": [
                {
                    "manifestId": "agent.semantic-protocols.providers.rust.asp-rust",
                    "manifestDigest": "sha256:" + "a" * 64,
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "binary": "asp-rust",
                    "providerCommandPrefix": [],
                    "resolvedBinary": "/nix/store/example/bin/asp-rust",
                    "argv": ["/nix/store/example/bin/asp-rust"],
                    "health": {"status": "available"},
                }
            ],
        }

    def test_runtime_profiles_accept_fixed_provider_argv(self):
        self.assertEqual([], self.validation_errors(self.valid_profiles()))

    def test_runtime_profiles_allow_missing_health_without_resolved_binary(self):
        profiles = self.valid_profiles()
        provider = profiles["providers"][0]
        provider.pop("resolvedBinary")
        provider["argv"] = []
        provider["health"] = {
            "status": "missing",
            "reason": "`asp-rust` was not found on PATH",
        }

        self.assertEqual([], self.validation_errors(profiles))

    def test_runtime_profiles_reject_unknown_provider_fields(self):
        profiles = self.valid_profiles()
        profiles["providers"][0]["pathHint"] = "runtime/bin"

        self.assertTrue(
            any(
                "Additional properties are not allowed" in error
                for error in self.validation_errors(profiles)
            )
        )


if __name__ == "__main__":
    unittest.main()
