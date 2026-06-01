"""Validate the root-owned agent hook decision schema."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_decision(reason_kind: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.agent-hook-decision",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.agent-hooks",
        "protocolVersion": "1",
        "platform": "codex",
        "event": "pre-tool",
        "decision": "deny",
        "reasonKind": reason_kind,
        "languageIds": ["typescript"],
        "subject": {
            "toolName": "Bash",
            "command": "ts-harness search text location.path owner tests --json .",
        },
        "routes": [
            {
                "languageId": "typescript",
                "providerId": "ts-harness",
                "binary": "ts-harness",
                "kind": "text",
                "argv": [
                    "ts-harness",
                    "search",
                    "text",
                    "location.path",
                    "owner",
                    "tests",
                    "--view",
                    "seeds",
                    ".",
                ],
            }
        ],
        "message": "Use compact search output for agent exploration.",
    }


class SemanticAgentHookDecisionSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-agent-hook-decision.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, decision: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(decision)]

    def test_direct_source_read_reason_kind_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_decision("direct-source-read")))

    def test_unknown_reason_kind_is_rejected(self) -> None:
        errors = self.validation_errors(minimal_decision("json-is-large"))

        self.assertTrue(any("is not one of" in message for message in errors))

    def test_language_packages_do_not_copy_root_owned_schema(self) -> None:
        for package_path in (
            _REPO_ROOT
            / "languages"
            / "rust-lang-project-harness"
            / "schemas"
            / "semantic-agent-hook-decision.v1.schema.json",
            _REPO_ROOT
            / "languages"
            / "typescript-lang-project-harness"
            / "schemas"
            / "semantic-agent-hook-decision.v1.schema.json",
            _REPO_ROOT
            / "languages"
            / "python-lang-project-harness"
            / "schemas"
            / "semantic-agent-hook-decision.v1.schema.json",
        ):
            with self.subTest(path=str(package_path.relative_to(_REPO_ROOT))):
                self.assertFalse(package_path.exists())


if __name__ == "__main__":
    unittest.main()
