"""Schema checks for prompt-only live agent sandtable scenarios."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _live_agent_scenario() -> dict[str, Any]:
    return {
        "id": "typescript.effect-live",
        "language": "typescript",
        "workdir": ".",
        "skipUnlessEnv": ["ANTHROPIC_AUTH_TOKEN"],
        "liveAgent": {
            "client": "claude",
            "outputFormat": "stream-json",
            "includeHookEvents": True,
            "verbose": True,
            "useRepoClaudeSettings": True,
            "timeoutSeconds": 120,
            "expect": {
                "agentAnswer": {
                    "required": True,
                    "afterLastToolUse": True,
                },
                "pipeFlow": {
                    "requiredStages": ["search-prime", "search-pipe"],
                    "forbiddenStages": ["repeated-prime"],
                },
            },
        },
        "evidence": {
            "source": "real-trigger",
            "deepQuestionCases": [
                {
                    "id": "effect-question",
                    "question": (
                        "Where should Effect concurrency behavior be located "
                        "before editing?"
                    ),
                    "stepIds": ["effect-question"],
                    "queryTerms": ["Effect", "concurrency", "Fiber"],
                    "audit": {
                        "maxAspCommands": 3,
                        "maxSearchCommands": 2,
                        "maxQueryCommands": 1,
                        "maxRepeatedCommands": 0,
                        "requiresGraphSignals": True,
                    },
                }
            ],
        },
    }


class LiveAgentScenarioSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = Draft202012Validator(_load_json(SCHEMA_PATH))

    def validation_errors(self, scenario: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(scenario)]

    def test_live_agent_prompt_only_scenario_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(_live_agent_scenario()))

    def test_live_agent_rejects_scripted_prompt_field(self) -> None:
        scenario = _live_agent_scenario()
        scenario["liveAgent"]["prompt"] = (
            "Run asp typescript search prime --workspace . --view seeds"
        )

        errors = self.validation_errors(scenario)

        self.assertTrue(
            any("Additional properties are not allowed" in error for error in errors)
        )


if __name__ == "__main__":
    unittest.main()
