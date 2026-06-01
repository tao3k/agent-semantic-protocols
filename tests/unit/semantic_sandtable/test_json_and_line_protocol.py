"""JSON expectation and line-protocol validation tests."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


class JsonAndLineProtocolTests(unittest.TestCase):
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

    def test_line_protocol_rejects_rank_prefixed_path_values(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-path-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-path",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-owner]')\n"
                                        "print('|hit path=0:src/a.ts line=1 kind=text')\n"
                                        "print('|edge 0:src/a.ts -import-> 0:src/b.ts')\n"
                                        "print('|find TS-AGENT-R007 x1 at=0:src/a.ts')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn("non-path prefix", result.steps[0].errors[0])

    def test_line_protocol_rejects_rank_prefixed_synthesis_paths(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-synthesis-path-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-synthesis-path",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-prime]')\n"
                                        "print('|synthesis algorithm=owner-rank-frontier "
                                        "scope=prime highImpactOwners=0:src/a.ts "
                                        "seeds=owner:0:src/b.ts')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn("non-path prefix", result.steps[0].errors[0])

    def test_line_protocol_allows_locations_without_path_prefix_drift(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.good-path-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "good-path",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-owner]')\n"
                                        "print('|owner src/a.ts locations=42:17,77:9 "
                                        "next=owner:src/b.ts')\n"
                                        "print('|edge O:src/a.ts -import-> O:src/b.ts')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status)

    def test_line_protocol_rejects_path_line_location_locators(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-path-line-locator",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-locator",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-text]')\n"
                                        "print('|hit src/a.ts:42:17 owner=src/a.ts kind=text')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn("mixes path and line/column", result.steps[0].errors[0])

    def test_line_protocol_ignores_rank_like_text_snippets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.text-snippet",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "snippet",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-ingest]')\n"
                                        "print('|hit path=src/a.ts line=3 "
                                        "kind=text text=\"/3:7/u\"')\n"
                                        "print('|hit path=src/b.ts line=4 "
                                        "kind=text text=\"path=src\\\\/consumer\\\\.ts\"')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
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
