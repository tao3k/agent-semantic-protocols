from __future__ import annotations

import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.runner import (
    coverage_report,
    discover_scenarios,
    main,
    run_scenario,
)


class SemanticSandtableRunnerTests(unittest.TestCase):
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

    def test_stdout_json_schema_validates_whole_payload_and_nested_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            schema_dir = repo_root / "schemas"
            schema_dir.mkdir()
            (schema_dir / "whole.schema.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "type": "object",
                        "required": ["agentHookDecision"],
                        "properties": {
                            "agentHookDecision": {"type": "object"},
                        },
                    }
                ),
                encoding="utf-8",
            )
            (schema_dir / "decision.schema.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "type": "object",
                        "required": ["reasonKind"],
                        "properties": {
                            "reasonKind": {"const": "direct-source-read"},
                        },
                    }
                ),
                encoding="utf-8",
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.hook-schema",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "schema",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        "print(json.dumps({'agentHookDecision': "
                                        "{'reasonKind': 'direct-source-read'}}))"
                                    ),
                                ],
                                "expect": {
                                    "stdoutJsonSchema": "schemas/whole.schema.json",
                                    "stdoutJsonSchemaAt": {
                                        "agentHookDecision": (
                                            "schemas/decision.schema.json"
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

    def test_stdout_json_paths_support_array_indexes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.registry-path",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "array-index",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        "print(json.dumps({'languages': "
                                        "[{'languageId': 'python'}]}))"
                                    ),
                                ],
                                "expect": {
                                    "stdoutJsonEquals": {
                                        "languages.0.languageId": "python"
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status)

    def test_coverage_report_uses_schema_surfaces_and_step_coverage(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            schema_dir = repo_root / "schemas"
            schema_dir.mkdir()
            (schema_dir / "semantic-sandtable-scenario.v1.schema.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "type": "object",
                        "required": ["id", "language", "workdir", "steps"],
                        "properties": {
                            "id": {"type": "string"},
                            "language": {"type": "string"},
                            "workdir": {"type": "string"},
                            "coverage": {
                                "$ref": "#/$defs/coverageList",
                            },
                            "steps": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": {"type": "string"},
                                        "command": {"type": "array"},
                                        "coverage": {
                                            "$ref": "#/$defs/coverageList",
                                        },
                                    },
                                },
                            },
                        },
                        "$defs": {
                            "coverageList": {
                                "type": "array",
                                "items": {
                                    "enum": [
                                        "search-flow",
                                        "deps-query",
                                        "codex-hooks",
                                    ]
                                },
                            }
                        },
                    }
                ),
                encoding="utf-8",
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.search",
                        "language": "python",
                        "workdir": ".",
                        "coverage": ["search-flow"],
                        "steps": [
                            {
                                "id": "deps",
                                "coverage": ["deps-query"],
                                "command": ["missing-binary"],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            report = coverage_report(repo_root, [scenario_path])

        self.assertEqual(1, report.scenario_count)
        self.assertEqual({"python"}, report.language_ids)
        self.assertIn("search-flow", report.surfaces)
        self.assertIn("deps-query", report.surfaces)
        self.assertEqual(["codex-hooks"], report.missing)
        self.assertEqual({"python.search:deps"}, report.surfaces["deps-query"].step_ids)

    def test_coverage_cli_does_not_execute_scenario_commands(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.coverage",
                        "language": "python",
                        "workdir": ".",
                        "coverage": ["search-flow"],
                        "steps": [{"id": "never-run", "command": ["missing-binary"]}],
                    }
                ),
                encoding="utf-8",
            )
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(
                    ["--coverage", "--repo-root", str(repo_root), "scenario.json"]
                )

        self.assertEqual(0, exit_code)
        self.assertIn("[coverage]", stdout.getvalue())
        self.assertIn("|surface search-flow", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
