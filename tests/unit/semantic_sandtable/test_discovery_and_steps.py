"""Discovery, schema-loading, capture, and stdin behavior tests."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.scenario_io import discover_scenarios
from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


class DiscoveryAndStepRunnerTests(unittest.TestCase):
    def test_discovery_ignores_generated_hidden_state_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_dir = repo_root / "sandtables" / "rust"
            scenario_dir.mkdir(parents=True)
            scenario_path = scenario_dir / "flow.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.flow",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [],
                    }
                ),
                encoding="utf-8",
            )
            state_dir = (
                repo_root
                / "sandtables"
                / "fixtures"
                / "demo"
                / ".codex"
                / "harness-state"
            )
            state_dir.mkdir(parents=True)
            (state_dir / "project.json").write_text(
                json.dumps({"id": "project"}),
                encoding="utf-8",
            )
            (repo_root / "sandtables" / "coverage-policy.json").write_text(
                json.dumps(
                    {
                        "schemaVersion": "semantic-sandtable-coverage-policy.v1",
                        "languages": [],
                    }
                ),
                encoding="utf-8",
            )
            receipt_dir = repo_root / "sandtables" / "receipts" / "rust"
            receipt_dir.mkdir(parents=True)
            (receipt_dir / "flow.receipt.json").write_text(
                json.dumps(
                    {
                        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
                        "schemaVersion": "1",
                    }
                ),
                encoding="utf-8",
            )

            self.assertEqual([scenario_path], discover_scenarios(repo_root, []))

    def test_schema_validation_fails_before_running_steps(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            schema_dir = repo_root / "schemas"
            schema_dir.mkdir()
            (schema_dir / "semantic-sandtable-scenario.v1.schema.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "type": "object",
                        "additionalProperties": False,
                        "required": ["id"],
                        "properties": {"id": {"type": "string"}},
                    }
                ),
                encoding="utf-8",
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps({"id": "bad.schema", "unexpected": True}),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertEqual([], result.steps)
        self.assertIn("failed schema validation", result.errors[0])

    def test_capture_expansion_and_stdin_command_pipe(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            workdir = repo_root / "workspace"
            workdir.mkdir()
            helper = repo_root / "helper.py"
            helper.write_text(
                "\n".join(
                    [
                        "from __future__ import annotations",
                        "import sys",
                        "mode = sys.argv[1]",
                        "if mode == 'seed':",
                        "    print('[seed]')",
                        "    print('|owner src/demo.py')",
                        "elif mode == 'inspect':",
                        "    stdin = sys.stdin.read()",
                        "    print('[inspect]')",
                        "    print(f'|owner {sys.argv[2]}')",
                        "    print(f'|stdin_lines {len(stdin.splitlines())}')",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.test-flow",
                        "language": "python",
                        "workdir": {"relative": "workspace"},
                        "steps": [
                            {
                                "id": "seed",
                                "command": [sys.executable, str(helper), "seed"],
                                "capture": {"owner": "\\|owner ([^\\s]+)"},
                                "expect": {
                                    "lineProtocol": True,
                                    "stdoutContains": ["[seed]", "|owner src/demo.py"],
                                },
                            },
                            {
                                "id": "inspect",
                                "command": [
                                    sys.executable,
                                    str(helper),
                                    "inspect",
                                    "{owner}",
                                ],
                                "stdinCommand": [sys.executable, str(helper), "seed"],
                                "expect": {
                                    "lineProtocol": True,
                                    "stdoutContains": [
                                        "[inspect]",
                                        "|owner src/demo.py",
                                        "|stdin_lines 2",
                                    ],
                                },
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status)
        self.assertEqual(["pass", "pass"], [step.status for step in result.steps])

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
                                "command": [sys.executable, "-c", "print('[ok]')"],
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
                                "command": [sys.executable, "-c", "print('[ok]')"],
                                "stdinCommand": [
                                    sys.executable,
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
                                    sys.executable,
                                    "-c",
                                    (
                                        "from pathlib import Path; "
                                        f"Path({str(marker)!r}).write_text('ran')"
                                    ),
                                ],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with patch.dict(os.environ, {}, clear=True):
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
                            "SANDTABLE_LIVE_ROOT": str(live_root),
                        },
                        "workdir": {"env": "SANDTABLE_LIVE_ROOT"},
                        "skipUnlessEnv": ["ASP_LIVE_CLAUDE_CLI"],
                        "steps": [
                            {
                                "id": "touch-marker",
                                "command": [
                                    sys.executable,
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

            with patch.dict(os.environ, {}, clear=True):
                result = run_scenario(repo_root, scenario_path)

            self.assertEqual("pass", result.status)
            self.assertEqual(["pass"], [step.status for step in result.steps])
            self.assertTrue(marker.exists())

    def test_workdir_git_clones_into_sandtable_repo_cache(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            origin = repo_root / "origin"
            origin.mkdir()
            subprocess.run(
                ["git", "init", str(origin)], check=True, capture_output=True
            )
            (origin / "README.md").write_text("fixture\n", encoding="utf-8")
            subprocess.run(
                ["git", "-C", str(origin), "add", "README.md"],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(origin),
                    "-c",
                    "user.name=Sandtable",
                    "-c",
                    "user.email=sandtable@example.invalid",
                    "commit",
                    "-m",
                    "fixture",
                ],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "-C", str(origin), "tag", "v1"],
                check=True,
                capture_output=True,
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "root.cached-git-workdir",
                        "language": "root",
                        "workdir": {
                            "git": {
                                "url": origin.as_uri(),
                                "ref": "v1",
                                "depth": 1,
                                "cacheKey": "fixture-v1",
                                "subdir": ".",
                            }
                        },
                        "steps": [
                            {
                                "id": "touch-marker",
                                "command": [
                                    sys.executable,
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

            result = run_scenario(repo_root, scenario_path)
            cache_checkout = repo_root / ".cache" / "sandtable-repos" / "fixture-v1"

            self.assertEqual("pass", result.status)
            self.assertEqual(cache_checkout.resolve(), result.workdir)
            self.assertTrue((cache_checkout / ".git").exists())
            self.assertTrue((cache_checkout / "ran").exists())


if __name__ == "__main__":
    unittest.main()
