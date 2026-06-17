"""Validate the root-owned agent hook decision schema."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_decision(reason_kind: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.hook",
        "protocolVersion": "1",
        "platform": "codex",
        "event": "pre-tool",
        "decision": "deny",
        "reasonKind": reason_kind,
        "languageIds": ["typescript"],
        "subject": {
            "toolName": "Bash",
            "command": "ts-harness search fzf location.path owner tests --json .",
        },
        "routes": [
            {
                "languageId": "typescript",
                "providerId": "ts-harness",
                "binary": "ts-harness",
                "kind": "fzf",
                "argv": [
                    "ts-harness",
                    "search",
                    "fzf",
                    "location.path",
                    "owner",
                    "tests",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
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

    def test_semantic_ast_patch_required_reason_kind_is_valid(self) -> None:
        self.assertEqual(
            [],
            self.validation_errors(minimal_decision("semantic-ast-patch-required")),
        )

    def test_source_directory_enumeration_reason_kind_is_valid(self) -> None:
        decision = minimal_decision("source-directory-enumeration")
        decision["fields"] = {"operationIntent": "directory-read"}
        decision["routes"][0]["kind"] = "ingest"  # type: ignore[index]

        self.assertEqual([], self.validation_errors(decision))

    def test_semantic_read_route_kind_is_valid(self) -> None:
        decision = minimal_decision("direct-source-read")
        decision["routes"][0] = {  # type: ignore[index]
            "languageId": "typescript",
            "providerId": "ts-harness",
            "binary": "ts-harness",
            "kind": "read",
            "argv": [
                "ts-harness",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "src/cli/agent-hooks.ts",
                ".",
            ],
        }
        self.assertEqual([], self.validation_errors(decision))

    def test_config_rule_id_field_is_valid(self) -> None:
        decision = minimal_decision("raw-broad-search")
        decision["fields"] = {"configRuleId": "deny-rust-rg"}

        self.assertEqual([], self.validation_errors(decision))

    def test_tool_surface_operation_fields_are_valid(self) -> None:
        decision = minimal_decision("direct-source-read")
        decision["fields"] = {
            "toolSurface": "codex-direct-read",
            "operationIntent": "direct-read",
        }

        self.assertEqual([], self.validation_errors(decision))

    def test_unknown_operation_intent_field_is_rejected(self) -> None:
        decision = minimal_decision("direct-source-read")
        decision["fields"] = {"operationIntent": "read-something"}
        errors = self.validation_errors(decision)

        self.assertTrue(any("is not one of" in message for message in errors))

    def test_invalid_config_rule_id_field_is_rejected(self) -> None:
        decision = minimal_decision("raw-broad-search")
        decision["fields"] = {"configRuleId": "DenyRustRg"}
        errors = self.validation_errors(decision)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_provider_query_route_kind_is_valid(self) -> None:
        decision = minimal_decision("direct-source-read")
        decision["routes"][0] = {  # type: ignore[index]
            "languageId": "typescript",
            "providerId": "ts-harness",
            "binary": "ts-harness",
            "kind": "query",
            "argv": [
                "ts-harness",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "src/cli/agent-hooks.ts",
                ".",
            ],
        }
        self.assertEqual([], self.validation_errors(decision))

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
