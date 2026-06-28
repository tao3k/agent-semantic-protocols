"""Validate scenario-level parallel sandtable execution."""

from __future__ import annotations

import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.models import ScenarioResult, StepResult
from tools.semantic_sandtable.scenario_runner import (
    _run_loaded_scenario,
    _run_scenario_steps,
)


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
        observed_envs: list[dict[str, str]] = []

        def fake_run_step(**kwargs: object) -> StepResult:
            step = kwargs["step"]
            assert isinstance(step, dict)
            env = kwargs["env"]
            assert isinstance(env, dict)
            env["STEP_MUTATION"] = str(step["id"])
            observed_envs.append(env)
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

        self.assertEqual(3, len({id(env) for env in observed_envs}))
        self.assertLess(elapsed, 0.35)
        self.assertEqual(
            ["one", "two", "three"], [step.step_id for step in result.steps]
        )
        self.assertEqual(450, totals["elapsedMs"])

    def test_scenario_isolation_sets_default_home_cache_and_tmp(self) -> None:
        observed_envs: list[dict[str, str]] = []
        observed_home_exists: list[bool] = []

        def fake_run_step(**kwargs: object) -> StepResult:
            step = kwargs["step"]
            env = kwargs["env"]
            assert isinstance(step, dict)
            assert isinstance(env, dict)
            observed_envs.append(env)
            observed_home_exists.append(Path(env["HOME"]).exists())
            return StepResult(
                scenario_id="python.large-repo",
                step_id=str(step["id"]),
                command=["fake"],
                status="pass",
                exit_code=0,
                elapsed_ms=1,
                stdout_lines=0,
                stderr_lines=0,
                stdout_bytes=0,
                stderr_bytes=0,
            )

        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            with patch(
                "tools.semantic_sandtable.scenario_runner.run_step",
                side_effect=fake_run_step,
            ):
                result = _run_loaded_scenario(
                    repo_root,
                    repo_root / "scenario.json",
                    {
                        "id": "python.large/repo",
                        "language": "python",
                        "steps": [{"id": "one"}, {"id": "two"}],
                        "execution": {"mode": "parallel", "maxConcurrentSteps": 2},
                    },
                )

        self.assertEqual("pass", result.status)
        self.assertEqual(2, len(observed_envs))
        self.assertEqual(2, len({id(env) for env in observed_envs}))
        homes = {env["HOME"] for env in observed_envs}
        self.assertEqual(1, len(homes))
        home = Path(next(iter(homes)))
        self.assertEqual([True, True], observed_home_exists)
        self.assertEqual(
            home / ".local" / "bin", Path(observed_envs[0]["PATH"].split(":")[0])
        )
        self.assertEqual(str(home.parent / "tmp"), observed_envs[0]["TMPDIR"])
        self.assertEqual(str(home.parent / "cache"), observed_envs[0]["XDG_CACHE_HOME"])
        self.assertEqual(
            str(home.parent / "cargo-target"), observed_envs[0]["CARGO_TARGET_DIR"]
        )
        self.assertIn("isolation", result.evidence)
        isolation = result.evidence["isolation"]
        self.assertTrue(isolation["enabled"])
        self.assertEqual("scenario", isolation["scope"])
        self.assertIn(
            "$ASP_REPO_ROOT/.cache/agent-semantic-protocol/sandtable/runs/",
            isolation["paths"]["root"],
        )
        self.assertEqual("$HOME", isolation["paths"]["home"])
        self.assertNotIn(str(home.parent.parent.parent.parent), str(isolation))

    def test_scenario_isolation_can_be_disabled(self) -> None:
        observed_envs: list[dict[str, str]] = []

        def fake_run_step(**kwargs: object) -> StepResult:
            step = kwargs["step"]
            env = kwargs["env"]
            assert isinstance(step, dict)
            assert isinstance(env, dict)
            observed_envs.append(env)
            return StepResult(
                scenario_id="python.no-isolation",
                step_id=str(step["id"]),
                command=["fake"],
                status="pass",
                exit_code=0,
                elapsed_ms=1,
                stdout_lines=0,
                stderr_lines=0,
                stdout_bytes=0,
                stderr_bytes=0,
            )

        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            with patch(
                "tools.semantic_sandtable.scenario_runner.run_step",
                side_effect=fake_run_step,
            ):
                result = _run_loaded_scenario(
                    repo_root,
                    repo_root / "scenario.json",
                    {
                        "id": "python.no-isolation",
                        "language": "python",
                        "isolation": {"enabled": False},
                        "steps": [{"id": "one"}],
                    },
                )

        self.assertEqual("pass", result.status)
        self.assertEqual(1, len(observed_envs))
        self.assertNotIn("ASP_SANDBOX_ROOT", observed_envs[0])
        self.assertNotIn("isolation", result.evidence)

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
                command,
                0,
                stdout=(
                    "workspaceBin=/repo/.bin/py-harness "
                    "installedPath=/tmp/sandtable-home/.local/bin/py-harness\n"
                ),
                stderr="",
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
        preflight_records = result.evidence["providerPreflight"][
            "installFromWorkspaceLanguages"
        ]
        self.assertEqual(
            ["python", "julia"],
            [record["language"] for record in preflight_records],
        )
        self.assertEqual("$ASP_REPO_ROOT", preflight_records[0]["command"][-1])
        self.assertIn("$ASP_REPO_ROOT/.bin/py-harness", preflight_records[0]["stdout"])
        self.assertIn("$HOME/.local/bin/py-harness", preflight_records[0]["stdout"])
        self.assertNotIn("/repo", str(preflight_records))
        self.assertNotIn("/tmp/sandtable-home", str(preflight_records))

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
                command,
                1,
                stdout="",
                stderr="stale rpath in /repo/.bin/asp-julia-harness",
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
            [
                "providerPreflight installFromWorkspace failed for julia: "
                "stale rpath in $ASP_REPO_ROOT/.bin/asp-julia-harness"
            ],
            result.errors,
        )
        self.assertNotIn("/repo", str(result.evidence))


if __name__ == "__main__":
    unittest.main()
