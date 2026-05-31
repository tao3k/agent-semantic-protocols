from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.runner import run_scenario


class SemanticSandtableRunnerTests(unittest.TestCase):
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

    def test_stdout_json_expectations_assert_hook_decisions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.hook-json",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "deny",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('{\"hookSpecificOutput\":"
                                        "{\"permissionDecision\":\"deny\","
                                        "\"permissionDecisionReason\":"
                                        "\"[flow] blocked=read-rs path=src/lib.rs\"}}')"
                                    ),
                                ],
                                "expect": {
                                    "stdoutJsonEquals": {
                                        "hookSpecificOutput.permissionDecision": "deny"
                                    },
                                    "stdoutJsonContains": {
                                        "hookSpecificOutput.permissionDecisionReason": (
                                            "blocked=read-rs"
                                        )
                                    },
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status)


if __name__ == "__main__":
    unittest.main()
