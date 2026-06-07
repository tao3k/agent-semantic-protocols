"""Validate large-library deep question sandtable coverage."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
RUST_MATRIX_PATHS = [
    REPO_ROOT / "sandtables" / "rust" / "tokio-intent-matrix.json",
    REPO_ROOT / "sandtables" / "rust" / "bytes-intent-matrix.json",
    REPO_ROOT / "sandtables" / "rust" / "ignore-intent-matrix.json",
]
LIVE_TOKIO_PATH = (
    REPO_ROOT / "sandtables" / "rust" / "tokio-claude-deep-question-flow.json"
)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


class DeepQuestionCaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = Draft202012Validator(_load_json(SCHEMA_PATH))

    def assert_valid_scenario(self, scenario: dict[str, Any]) -> None:
        errors = [error.message for error in self.validator.iter_errors(scenario)]
        self.assertEqual([], errors)

    def test_rust_large_library_matrices_define_ten_deep_questions(self) -> None:
        total_questions = 0
        for path in RUST_MATRIX_PATHS:
            scenario = _load_json(path)
            self.assert_valid_scenario(scenario)
            evidence = scenario["evidence"]
            step_ids = {step["id"] for step in scenario["steps"]}
            deep_questions = evidence["deepQuestionCases"]
            self.assertGreaterEqual(len(deep_questions), 3)
            total_questions += len(deep_questions)

            for question in deep_questions:
                self.assertTrue(question["question"].strip())
                self.assertGreaterEqual(len(question["queryTerms"]), 3)
                self.assertTrue(set(question["stepIds"]).issubset(step_ids))
                audit = question["audit"]
                self.assertLessEqual(
                    audit["maxSearchCommands"], audit["maxAspCommands"]
                )
                self.assertLessEqual(audit["maxQueryCommands"], audit["maxAspCommands"])
                self.assertEqual(0, audit["maxRepeatedCommands"])
                self.assertTrue(audit["requiresGraphSignals"])
                self.assertTrue(audit["requiresQuerySet"])

        self.assertEqual(10, total_questions)

    def test_live_tokio_claude_deep_question_is_env_gated(self) -> None:
        scenario = _load_json(LIVE_TOKIO_PATH)
        self.assert_valid_scenario(scenario)
        self.assertEqual(
            [
                "ANTHROPIC_AUTH_TOKEN",
            ],
            scenario["skipUnlessEnv"],
        )
        self.assertEqual(
            {
                "url": "https://github.com/tokio-rs/tokio.git",
                "ref": "tokio-1.52.3",
                "depth": 1,
                "cacheKey": "tokio-1.52.3",
                "subdir": ".",
            },
            scenario["workdir"]["git"],
        )
        step = scenario["steps"][0]
        self.assertEqual(
            {"mode": "parallel", "maxConcurrentSteps": 1}, scenario["execution"]
        )
        self.assertEqual(1, len(scenario["steps"]))
        self.assertEqual(120000, scenario["budgets"]["maxTotalElapsedMsWarn"])
        self.assertEqual(
            [120],
            [step["timeoutSeconds"] for step in scenario["steps"]],
        )
        self.assertEqual("agent-sdk", step["kind"])
        self.assertEqual("claude", step["agentSdk"]["client"])
        self.assertTrue(step["agentSdk"]["includeHookEvents"])
        self.assertEqual(["Bash"], step["agentSdk"]["allowedTools"])
        self.assertTrue(step["agentSdk"]["requireAspBashCommands"])
        self.assertTrue(step["agentSdk"]["useRepoClaudeSettings"])
        self.assertEqual(9, step["agentSdk"]["maxTurns"])
        step_ids = {step["id"] for step in scenario["steps"]}
        self.assertEqual(1, len(scenario["evidence"]["deepQuestionCases"]))
        for deep_question in scenario["evidence"]["deepQuestionCases"]:
            self.assertIn("Tokio 1.52.3", deep_question["question"])
            self.assertTrue(set(deep_question["stepIds"]).issubset(step_ids))
            self.assertTrue(deep_question["audit"]["requiresComplexPipeFlow"])
            self.assertTrue(deep_question["audit"]["requiresTokenCost"])
            expected_flow = deep_question["expectedAspFlow"]
            self.assertTrue(
                any(
                    command.startswith("asp rust search pipe ")
                    for command in expected_flow["canonicalCommands"]
                )
            )
            self.assertIn(
                "asp rust search prime --view seeds .",
                expected_flow["canonicalCommands"],
            )
            self.assertIn("search-prime", expected_flow["requiredStages"])
            self.assertIn("search-pipe", expected_flow["requiredStages"])
            self.assertIn("query-selector", expected_flow["requiredStages"])
            self.assertNotIn("search-reasoning", expected_flow["requiredStages"])
            self.assertIn("repeated-prime", expected_flow["forbiddenStages"])
        for step in scenario["steps"]:
            pipe_flow = step["expect"]["pipeFlow"]
            self.assertEqual(
                scenario["evidence"]["deepQuestionCases"][0]["expectedAspFlow"][
                    "requiredStages"
                ],
                pipe_flow["requiredStages"],
            )
            self.assertEqual(8, pipe_flow["maxAspCommands"])
            self.assertEqual(4, pipe_flow["maxSearchCommands"])
            self.assertEqual(4, pipe_flow["maxQueryCommands"])
            self.assertEqual(0, pipe_flow["maxRepeatedCommands"])
            self.assertEqual(1, pipe_flow["maxSearchPipeCommands"])
            self.assertEqual(1, pipe_flow["maxSearchPrimeCommands"])
            self.assertIn("search-prime", pipe_flow["requiredStages"])
            self.assertIn("search-pipe", pipe_flow["requiredStages"])
            self.assertIn("query-selector", pipe_flow["requiredStages"])
            self.assertNotIn("search-reasoning", pipe_flow["requiredStages"])
            self.assertTrue(pipe_flow["requireComplexPipeFlow"])
            self.assertTrue(pipe_flow["requireTokenCost"])


if __name__ == "__main__":
    unittest.main()
