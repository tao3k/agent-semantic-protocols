"""JSON expectation and line-protocol validation tests."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class JsonExpectationTests(unittest.TestCase):
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

    def test_stdout_json_array_contains_matches_scalars_and_object_subsets(self) -> None:
            with tempfile.TemporaryDirectory() as tmp:
                repo_root = Path(tmp)
                scenario_path = repo_root / "scenario.json"
                scenario_path.write_text(
                    json.dumps(
                        {
                            "id": "typescript.search-synthesis-json",
                            "language": "typescript",
                            "workdir": ".",
                            "steps": [
                                {
                                    "id": "synthesis",
                                    "command": [
                                        sys.executable,
                                        "-c",
                                        (
                                            "import json; "
                                            "print(json.dumps({'searchSynthesis': "
                                            "{'frontierOwners': ['src/model.ts'], "
                                            "'seeds': [{'kind': 'owner', "
                                            "'target': 'src/model.ts', "
                                            "'reason': 'frontier'}]}}))"
                                        ),
                                    ],
                                    "expect": {
                                        "stdoutJsonArrayContains": {
                                            "searchSynthesis.frontierOwners": "src/model.ts",
                                            "searchSynthesis.seeds": {
                                                "kind": "owner",
                                                "target": "src/model.ts",
                                            },
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


if __name__ == "__main__":
    unittest.main()
