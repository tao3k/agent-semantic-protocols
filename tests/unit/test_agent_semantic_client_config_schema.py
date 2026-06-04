"""Schema tests for agent semantic client configuration."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


class SemanticAgentClientConfigSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "agent-semantic-client-config.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, config: dict) -> list[str]:
        return [error.message for error in self.validator.iter_errors(config)]

    def test_local_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.client-config",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "backend": {"mode": "local"},
            "local": {"providers": ["rust", "typescript", "python"]},
            "privacy": {"uploadPolicy": "none"},
        }

        self.assertEqual(self.validation_errors(config), [])

    def test_hybrid_cloud_config_is_valid(self) -> None:
        config = {
            "backend": {"mode": "hybrid"},
            "cloud": {
                "endpoint": "grpc+tls://api.agent-semantic-protocols.dev:443",
                "auth": "device",
            },
            "privacy": {"uploadPolicy": "semantic-index"},
        }

        self.assertEqual(self.validation_errors(config), [])

    def test_rejects_unknown_backend_mode(self) -> None:
        errors = self.validation_errors({"backend": {"mode": "magic"}})

        self.assertTrue(any("is not one of" in error for error in errors))

    def test_rejects_duplicate_local_providers(self) -> None:
        errors = self.validation_errors({"local": {"providers": ["rust", "rust"]}})

        self.assertTrue(any("non-unique elements" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
