"""Focused semantic sandtable discovery and step tests."""

from __future__ import annotations

from ._discovery_steps_common import (
    Path,
    discover_scenarios,
    json,
    run_scenario,
    tempfile,
    unittest,
)


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
                                "command": ["python", "../helper.py", "seed"],
                                "capture": {"owner": "\\|owner ([^\\s]+)"},
                                "expect": {
                                    "lineProtocol": True,
                                    "stdoutContains": ["[seed]", "|owner src/demo.py"],
                                },
                            },
                            {
                                "id": "inspect",
                                "command": [
                                    "python",
                                    "../helper.py",
                                    "inspect",
                                    "{owner}",
                                ],
                                "stdinCommand": ["python", "../helper.py", "seed"],
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
