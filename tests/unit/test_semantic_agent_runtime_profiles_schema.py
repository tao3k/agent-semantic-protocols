"""Validate runtime profile schema facts used by agent provider repair."""

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


class SemanticAgentRuntimeProfilesSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "semantic-agent-runtime-profiles.v1.schema.json"
        )
        with open(schema_path, "r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

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
                    "manifestId": "agent.semantic-protocols.providers.rust.rs-harness",
                    "manifestDigest": "sha256:" + "a" * 64,
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "binary": "rs-harness",
                    "providerCommandPrefix": [],
                    "resolvedBinary": "/nix/store/example/bin/rs-harness",
                    "argv": ["/nix/store/example/bin/rs-harness"],
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
            "reason": "`rs-harness` was not found on PATH",
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
