"""Schema contract tests for semantic-language registry descriptors."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def registry_with_descriptor(descriptor: dict[str, object]) -> dict[str, object]:
    return {
        "registryId": "agent.semantic-protocols.semantic-language-registry",
        "registryVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languages": [
            {
                "languageId": "rust",
                "providerId": "rs-harness",
                "binary": "rs-harness",
                "namespace": "agent.semantic-protocols.rust",
                "methods": [descriptor["method"]],
                "methodDescriptors": [descriptor],
                "schemas": [],
            }
        ],
    }


class SemanticLanguageRegistrySchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT
            / "schemas"
            / "semantic-language-registry.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, descriptor: dict[str, object]) -> list[str]:
        errors = self.validator.iter_errors(registry_with_descriptor(descriptor))
        return [error.message for error in errors]

    def test_agent_compact_method_can_omit_output_schema_when_no_json(self) -> None:
        errors = self.validation_errors(
            {
                "method": "agent/guide",
                "command": "agent",
                "supportsJson": False,
                "supportsCompact": True,
                "clients": ["codex"],
                "requiredOptions": ["--client codex"],
            }
        )

        self.assertEqual([], errors)

    def test_agent_json_method_requires_output_schema_ids(self) -> None:
        errors = self.validation_errors(
            {
                "method": "agent/hook",
                "command": "agent",
                "supportsJson": True,
                "supportsCompact": False,
                "clients": ["codex"],
            }
        )

        self.assertIn("'outputSchemaIds' is a required property", errors)


if __name__ == "__main__":
    unittest.main()
