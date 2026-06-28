"""Validate scenario-level parallel sandtable execution."""

from __future__ import annotations

import time
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.models import ScenarioResult, StepResult
from tools.semantic_sandtable.scenario_runner import _run_scenario_steps


class ScenarioRunnerParallelTests(unittest.TestCase):
    def test_parallel_execution_preserves_step_order(self) -> None:
        result = ScenarioResult(
            scenario_id="rust.parallel",
            language="rust",
            path=Path("scenario.json"),
            status="pass",
            workdir=Path("."),
        )
        steps = [{"id": "one"}, {"id": "two"}, {"id": "three"}]

        def fake_run_step(**kwargs: object) -> StepResult:
            step = kwargs["step"]
            assert isinstance(step, dict)
            time.sleep(0.15)
            return StepResult(
                scenario_id="rust.parallel",
                step_id=str(step["id"]),
                command=["fake"],
                status="pass",
                exit_code=0,
                elapsed_ms=150,
                stdout_lines=0,
                stderr_lines=0,
                stdout_bytes=0,
                stderr_bytes=0,
            )

        started = time.perf_counter()
        with patch(
            "tools.semantic_sandtable.scenario_runner.run_step",
            side_effect=fake_run_step,
        ):
            totals = _run_scenario_steps(
                repo_root=Path("."),
                workdir=Path("."),
                scenario_id="rust.parallel",
                steps=steps,
                env={},
                captures={},
                result=result,
                execution={"mode": "parallel", "maxConcurrentSteps": 3},
            )
        elapsed = time.perf_counter() - started

        self.assertLess(elapsed, 0.35)
        self.assertEqual(
            ["one", "two", "three"], [step.step_id for step in result.steps]
        )
        self.assertEqual(450, totals["elapsedMs"])

    def test_provider_preflight_installs_workspace_languages_once(self) -> None:
        import subprocess

        from tools.semantic_sandtable.scenario_runner import _apply_provider_preflight

        result = ScenarioResult(
            scenario_id="python.provider-preflight",
            language="python",
            path=Path("scenario.json"),
            status="pass",
            workdir=Path("."),
        )
        calls: list[tuple[list[str], dict[str, object]]] = []

        def fake_run(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[str]:
            calls.append((command, kwargs))
            return subprocess.CompletedProcess(
                command, 0, stdout="installed\n", stderr=""
            )

        with patch("subprocess.run", side_effect=fake_run):
            failed = _apply_provider_preflight(
                Path("/repo"),
                {
                    "providerPreflight": {
                        "installFromWorkspaceLanguages": ["python", "julia", "python"]
                    }
                },
                {"HOME": "/tmp/sandtable-home"},
                result,
            )

        self.assertFalse(failed)
        self.assertEqual("pass", result.status)
        self.assertEqual(2, len(calls))
        self.assertEqual(
            ["install", "language", "python", "--from-workspace"],
            calls[0][0][1:5],
        )
        self.assertEqual(
            ["install", "language", "julia", "--from-workspace"],
            calls[1][0][1:5],
        )
        self.assertEqual("/repo", calls[0][0][-1])
        self.assertEqual(Path("/repo"), calls[0][1]["cwd"])
        self.assertEqual("/tmp/sandtable-home", calls[0][1]["env"]["HOME"])
        self.assertEqual(
            ["python", "julia"],
            [
                record["language"]
                for record in result.evidence["providerPreflight"][
                    "installFromWorkspaceLanguages"
                ]
            ],
        )

    def test_provider_preflight_failure_marks_scenario_failed(self) -> None:
        import subprocess

        from tools.semantic_sandtable.scenario_runner import _apply_provider_preflight

        result = ScenarioResult(
            scenario_id="julia.provider-preflight",
            language="julia",
            path=Path("scenario.json"),
            status="pass",
            workdir=Path("."),
        )

        def fake_run(
            command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command, 1, stdout="", stderr="stale rpath"
            )

        with patch("subprocess.run", side_effect=fake_run):
            failed = _apply_provider_preflight(
                Path("/repo"),
                {"providerPreflight": {"installFromWorkspaceLanguages": ["julia"]}},
                {},
                result,
            )

        self.assertTrue(failed)
        self.assertEqual("fail", result.status)
        self.assertEqual(
            ["providerPreflight installFromWorkspace failed for julia: stale rpath"],
            result.errors,
        )


if __name__ == "__main__":
    unittest.main()
