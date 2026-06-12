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
LIVE_PROMPT_ONLY_PATHS = [
    LIVE_TOKIO_PATH,
    REPO_ROOT / "sandtables" / "rust" / "tokio-real-trigger-flow.json",
    REPO_ROOT / "sandtables" / "rust" / "tokio-task-abort-flow.json",
    REPO_ROOT / "sandtables" / "typescript" / "effect-claude-concurrency-flow.json",
]


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
                "ASP_LIVE_CLAUDE_CLI",
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
        self.assertEqual(
            {"mode": "parallel", "maxConcurrentSteps": 1}, scenario["execution"]
        )
        self.assertNotIn("steps", scenario)
        live_agent = scenario["liveAgent"]
        self.assertEqual(120000, scenario["budgets"]["maxTotalElapsedMsWarn"])
        self.assertEqual(120, live_agent["timeoutSeconds"])
        self.assertEqual("claude", live_agent["client"])
        self.assertTrue(live_agent["includeHookEvents"])
        self.assertTrue(live_agent["useRepoClaudeSettings"])
        self.assertNotIn("prompt", live_agent)
        self.assertNotIn("command", live_agent)
        self.assertNotIn("maxTurns", live_agent)
        self.assertNotIn("allowedTools", live_agent)
        self.assertNotIn("requireAspBashCommands", live_agent)
        self.assertEqual(
            scenario["evidence"]["deepQuestionCases"][0]["question"],
            "In Tokio 1.52.3, what do Vec<scalar> collection fields mean, and why are they not modeled as ordinary scalar fields?",
        )
        self.assertEqual(1, len(scenario["evidence"]["deepQuestionCases"]))
        for deep_question in scenario["evidence"]["deepQuestionCases"]:
            self.assertIn("Tokio 1.52.3", deep_question["question"])
            self.assertNotIn("asp rust", deep_question["question"])
            self.assertTrue(deep_question["audit"]["requiresComplexPipeFlow"])
            self.assertTrue(deep_question["audit"]["requiresTokenCost"])
            expected_flow = deep_question["expectedAspFlow"]
            self.assertIn("search-prime", expected_flow["requiredStages"])
            self.assertIn("search-pipe", expected_flow["requiredStages"])
            self.assertIn("query-selector", expected_flow["requiredStages"])
            self.assertNotIn("search-reasoning", expected_flow["requiredStages"])
            self.assertIn("repeated-prime", expected_flow["forbiddenStages"])
        pipe_flow = live_agent["expect"]["pipeFlow"]
        self.assertEqual(
            scenario["evidence"]["deepQuestionCases"][0]["expectedAspFlow"][
                "requiredStages"
            ],
            pipe_flow["requiredStages"],
        )
        self.assertEqual(3, pipe_flow["maxAspCommands"])
        self.assertEqual(2, pipe_flow["maxSearchCommands"])
        self.assertEqual(1, pipe_flow["maxQueryCommands"])
        self.assertEqual(0, pipe_flow["maxGuideCommands"])
        self.assertEqual(0, pipe_flow["maxRepeatedCommands"])
        self.assertEqual(1, pipe_flow["maxSearchPipeCommands"])
        self.assertEqual(1, pipe_flow["maxSearchPrimeCommands"])
        self.assertEqual(0, pipe_flow["maxReadLoopDuplicateSelectors"])
        self.assertEqual(0, pipe_flow["maxReadLoopAdjacentRangeWindows"])
        self.assertEqual(0, pipe_flow["maxReadLoopSameOwnerScans"])
        self.assertIn("search-prime", pipe_flow["requiredStages"])
        self.assertIn("search-pipe", pipe_flow["requiredStages"])
        self.assertIn("query-selector", pipe_flow["requiredStages"])
        self.assertNotIn("search-reasoning", pipe_flow["requiredStages"])
        self.assertIn("read-loop-risk", pipe_flow["forbiddenStages"])
        self.assertTrue(pipe_flow["requireComplexPipeFlow"])
        self.assertTrue(pipe_flow["requireTokenCost"])
        self.assertTrue(pipe_flow["requireSearchPipePrecision"])

    def test_live_agent_scenarios_are_prompt_only(self) -> None:
        for path in LIVE_PROMPT_ONLY_PATHS:
            with self.subTest(path=str(path.relative_to(REPO_ROOT))):
                scenario = _load_json(path)
                self.assert_valid_scenario(scenario)
                self.assertEqual(
                    ["ASP_LIVE_CLAUDE_CLI", "ANTHROPIC_AUTH_TOKEN"],
                    scenario["skipUnlessEnv"],
                )
                self.assertNotIn("steps", scenario)
                live_agent = scenario["liveAgent"]
                self.assertEqual("claude", live_agent["client"])
                self.assertTrue(live_agent["useRepoClaudeSettings"])
                self.assertTrue(live_agent["includeHookEvents"])
                self.assertNotIn("prompt", live_agent)
                self.assertNotIn("command", live_agent)
                self.assertNotIn("allowedTools", live_agent)
                self.assertNotIn("requireAspBashCommands", live_agent)
                questions = [
                    case["question"]
                    for case in scenario["evidence"].get("deepQuestionCases", [])
                ]
                self.assertEqual(1, len(questions))
                self.assertNotIn("asp ", questions[0])
                self.assertNotIn("Command ", questions[0])


if __name__ == "__main__":
    unittest.main()
