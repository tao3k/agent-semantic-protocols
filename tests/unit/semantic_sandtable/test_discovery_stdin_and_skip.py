"""Focused semantic sandtable discovery and step tests."""

from __future__ import annotations

from ._discovery_steps_common import (
    Path,
    json,
    os,
    patch,
    run_scenario,
    tempfile,
    unittest,
)


class DiscoveryAndStepRunnerTests(unittest.TestCase):
    def test_missing_capture_in_inline_stdin_fails_without_crashing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.missing-stdin-capture",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "stdin",
                                "command": ["python", "-c", "print('[ok]')"],
                                "stdin": "{missing}",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertEqual(["missing capture 'missing'"], result.steps[0].errors)

    def test_stdin_command_non_zero_fails_by_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.stdin-command-failure",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "stdin-command",
                                "command": ["python", "-c", "print('[ok]')"],
                                "stdinCommand": [
                                    "python",
                                    "-c",
                                    "import sys; sys.exit(7)",
                                ],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertEqual(["stdin command exited 7"], result.steps[0].errors)
        self.assertEqual(7, result.steps[0].exit_code)

    def test_skip_unless_env_marks_scenario_skip_before_running_steps(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            marker = repo_root / "should-not-run"
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "root.live-env-gate",
                        "language": "root",
                        "workdir": ".",
                        "skipUnlessEnv": ["ASP_LIVE_CLAUDE_CLI"],
                        "steps": [
                            {
                                "id": "touch-marker",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "from pathlib import Path; "
                                        "Path('should-not-run').write_text('ran')"
                                    ),
                                ],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with patch.dict(os.environ, {"PATH": os.environ.get("PATH", "")}, clear=True):
                result = run_scenario(repo_root, scenario_path)

        self.assertEqual("skip", result.status)
        self.assertEqual("missing env: ASP_LIVE_CLAUDE_CLI", result.skip_reason)
        self.assertEqual([], result.steps)
        self.assertFalse(marker.exists())

    def test_scenario_env_feeds_skip_gate_and_workdir_resolution(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            live_root = repo_root / "live-workdir"
            live_root.mkdir()
            marker = live_root / "ran"
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "root.live-env-inheritance",
                        "language": "root",
                        "env": {
                            "ASP_LIVE_CLAUDE_CLI": "1",
                            "SANDTABLE_LIVE_ROOT": "live-workdir",
                        },
                        "workdir": {"env": "SANDTABLE_LIVE_ROOT"},
                        "skipUnlessEnv": ["ASP_LIVE_CLAUDE_CLI"],
                        "steps": [
                            {
                                "id": "touch-marker",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "from pathlib import Path; "
                                        "Path('ran').write_text('ok')"
                                    ),
                                ],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with patch.dict(os.environ, {"PATH": os.environ.get("PATH", "")}, clear=True):
                result = run_scenario(repo_root, scenario_path)

            self.assertEqual("pass", result.status)
            self.assertEqual(["pass"], [step.status for step in result.steps])
            self.assertTrue(marker.exists())
