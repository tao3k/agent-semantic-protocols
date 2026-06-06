"""Schema tests for semantic sandtable scenarios."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


class SemanticSandtableScenarioSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
        )
        self.validator = Draft202012Validator(_load_json(schema_path))

    def validation_errors(self, scenario: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(scenario)]

    def test_guide_quality_route_command_assertions_are_valid(self) -> None:
        scenario: dict[str, object] = {
            "id": "typescript.codex-guide",
            "language": "typescript",
            "workdir": ".",
            "steps": [
                {
                    "id": "guide",
                    "command": ["node", "-e", "console.log('{}')"],
                    "expect": {
                        "guideQuality": {
                            "reasonKind": "raw-broad-search",
                            "languageId": "typescript",
                            "routeKind": "query",
                            "routeCommandContains": [
                                "ts-harness query --from-hook",
                                "--surface owners,tests",
                            ],
                            "routeCommandNotContains": [
                                "search query",
                                "--surface owner,tests",
                            ],
                            "outputContains": [
                                "entries=owner-query(O,Q=>items+tests+dependency-usage)"
                            ],
                            "outputNotContains": ["profiles" + "="],
                            "primeOutput": {
                                "requiresStructureStatus": True,
                                "entries": [
                                    "entries=owner-query(O,Q=>items+tests+dependency-usage)"
                                ],
                            },
                        }
                    },
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))

    def test_guide_quality_prime_output_entries_must_be_entries_lines(self) -> None:
        scenario: dict[str, object] = {
            "id": "typescript.codex-guide",
            "language": "typescript",
            "workdir": ".",
            "steps": [
                {
                    "id": "guide",
                    "command": ["node", "-e", "console.log('{}')"],
                    "expect": {
                        "guideQuality": {
                            "primeOutput": {
                                "entries": ["entries=ad-hoc-owner-map(O=>items)"],
                            }
                        }
                    },
                }
            ],
        }

        self.assertTrue(
            any("does not match" in error for error in self.validation_errors(scenario))
        )

    def test_agent_cli_claude_step_is_valid(self) -> None:
        scenario: dict[str, object] = {
            "id": "root.claude-cli",
            "language": "root",
            "workdir": ".",
            "skipUnlessEnv": ["ASP_LIVE_CLAUDE_CLI", "ANTHROPIC_AUTH_TOKEN"],
            "steps": [
                {
                    "id": "claude-print",
                    "kind": "agent-cli",
                    "agentCli": {
                        "client": "claude",
                        "binary": "claude",
                        "prompt": "Explain ASP hook install",
                        "outputFormat": "stream-json",
                        "inputFormat": "text",
                        "includePartialMessages": True,
                        "includeHookEvents": True,
                        "verbose": True,
                        "model": "deepseek-v4",
                        "env": {
                            "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                            "ANTHROPIC_AUTH_TOKEN": "${DEEPSEEK_API_KEY}",
                        },
                        "requiredEnv": ["ANTHROPIC_AUTH_TOKEN"],
                    },
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))

    def test_skip_unless_env_rejects_non_env_names(self) -> None:
        scenario: dict[str, object] = {
            "id": "root.claude-cli",
            "language": "root",
            "workdir": ".",
            "skipUnlessEnv": ["live-claude"],
            "steps": [
                {
                    "id": "claude-print",
                    "agentCli": {
                        "client": "claude",
                        "binary": "claude",
                        "prompt": "Explain ASP hook install",
                        "outputFormat": "text",
                    },
                }
            ],
        }

        self.assertTrue(
            any("does not match" in error for error in self.validation_errors(scenario))
        )

    def test_step_rejects_command_and_agent_cli_together(self) -> None:
        scenario: dict[str, object] = {
            "id": "root.claude-cli",
            "language": "root",
            "workdir": ".",
            "steps": [
                {
                    "id": "ambiguous",
                    "command": ["true"],
                    "agentCli": {
                        "client": "claude",
                        "binary": "claude",
                        "prompt": "Explain ASP hook install",
                        "outputFormat": "text",
                    },
                }
            ],
        }

        self.assertTrue(
            any(
                "{'id': 'ambiguous'" in error or "is valid under each" in error
                for error in self.validation_errors(scenario)
            )
        )

    def test_failure_frontier_comparison_evidence_is_valid(self) -> None:
        scenario = _load_json(
            _REPO_ROOT
            / "sandtables"
            / "fixtures"
            / "asp"
            / "failure-frontier-real-trigger-replay.json"
        )

        self.assertEqual([], self.validation_errors(scenario))

    def test_failure_frontier_comparison_rejects_unknown_threshold(self) -> None:
        scenario = _load_json(
            _REPO_ROOT
            / "sandtables"
            / "fixtures"
            / "asp"
            / "failure-frontier-real-trigger-replay.json"
        )
        evidence = scenario["evidence"]
        assert isinstance(evidence, dict)
        comparison = evidence["failureFrontierComparison"]
        assert isinstance(comparison, dict)
        thresholds = comparison["thresholds"]
        assert isinstance(thresholds, dict)
        thresholds["maxRawSourceWindows"] = 4

        errors = self.validation_errors(scenario)

        self.assertTrue(
            any("Additional properties are not allowed" in error for error in errors)
        )

    def test_semantic_sandtable_scenario_schema_copies_stay_synchronized(self) -> None:
        root_schema = _load_json(
            _REPO_ROOT / "schemas/semantic-sandtable-scenario.v1.schema.json"
        )
        schema_copies = sorted(
            path
            for path in (_REPO_ROOT / "languages").glob(
                "*/schemas/semantic-sandtable-scenario.v1.schema.json"
            )
        )

        self.assertTrue(schema_copies)
        self.assertEqual(
            {schema_path: root_schema for schema_path in schema_copies},
            {schema_path: _load_json(schema_path) for schema_path in schema_copies},
        )

if __name__ == "__main__":
    unittest.main()
