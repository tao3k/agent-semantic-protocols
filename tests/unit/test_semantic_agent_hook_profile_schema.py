"""Validate the shared agent hook profile registry schema."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def command(argv: list[str], stdin_mode: str | None = None) -> dict[str, object]:
    template: dict[str, object] = {"text": " ".join(argv), "argv": argv}
    if stdin_mode is not None:
        template["stdinMode"] = stdin_mode
    return template


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
                    "directSourceRead": "block",
                    "bulkSourceDump": "block",
                    "rawSourceSearch": "block",
                    "agentSearchJson": "block",
                    "blockDirectRead": True,
                    "blockBroadRawSearch": True,
                    "blockAgentSearchJson": True,
                    "requirePrimeBeforeEdit": True,
                },
                "commands": {
                    "prime": command(["ts-harness", "search", "prime", "."]),
                    "owner": command(
                        ["ts-harness", "search", "owner", "{path}", "."]
                    ),
                    "fzf": command(
                        [
                            "ts-harness",
                            "search",
                            "fzf",
                            "{query}",
                            "owner",
                            "tests",
                            "--view",
                            "seeds",
                            ".",
                        ]
                    ),
                    "ingest": command(
                        [
                            "ts-harness",
                            "search",
                            "ingest",
                            "owner",
                            "tests",
                            "--view",
                            "seeds",
                            ".",
                        ],
                        "pipe-candidates",
                    ),
                    "checkChanged": command(
                        ["ts-harness", "check", "--changed", "."]
                    ),
                    "guide": command(["ts-harness", "agent", "guide", "."]),
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

    def test_provider_command_prefix_supports_workspace_managed_provider(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        profile["languageId"] = "julia"
        profile["providerId"] = "julia-project-harness"
        profile["binary"] = "julia-project-harness"
        profile["providerCommandPrefix"] = [
            "julia",
            "--project=languages/JuliaLangProjectHarness.jl",
            "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl",
        ]
        profile["namespace"] = (
            "agent.semantic-protocols.languages.julia.julia-project-harness"
        )
        profile["sourceExtensions"] = [".jl"]
        registry["profiles"] = [profile]
        self.assertEqual([], self.validation_errors(registry))

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

    def test_command_templates_require_agent_text(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        del profile["commands"]["prime"]["text"]
        registry["profiles"] = [profile]

        errors = self.validation_errors(registry)

        self.assertTrue(any("'text' is a required property" in message for message in errors))

    def test_legacy_text_route_is_rejected(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        profile["commands"]["text"] = profile["commands"].pop("fzf")
        registry["profiles"] = [profile]

        errors = self.validation_errors(registry)

        self.assertTrue(any("'fzf' is a required property" in message for message in errors))

    def test_guide_command_remains_optional_for_legacy_profiles(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        del profile["commands"]["guide"]
        registry["profiles"] = [profile]

        self.assertEqual([], self.validation_errors(registry))

    def test_action_policy_rejects_unknown_modes(self) -> None:
        registry = minimal_registry()
        profile = copy.deepcopy(registry["profiles"][0])
        profile["policy"]["rawSourceSearch"] = "warn"
        registry["profiles"] = [profile]

        errors = self.validation_errors(registry)

        self.assertTrue(any("'warn' is not one of" in message for message in errors))

    def test_provider_profile_resources_are_canonical(self) -> None:
        profile_paths = sorted(
            _REPO_ROOT.glob(
                "languages/**/semantic-agent-hook-profile.*.v1.json"
            )
        )
        self.assertGreater(len(profile_paths), 0)
        for path in profile_paths:
            with self.subTest(path=str(path.relative_to(_REPO_ROOT))):
                with path.open("r", encoding="utf-8") as handle:
                    registry = json.load(handle)
                self.assertEqual([], self.validation_errors(registry))


if __name__ == "__main__":
    unittest.main()
