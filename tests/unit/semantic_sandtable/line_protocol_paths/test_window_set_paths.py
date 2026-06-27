"""Validate line-protocol path rules for synthesis window sets."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolWindowSetPathTests(unittest.TestCase):
    def test_line_protocol_validates_window_set_path_values(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.good-window-set-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "good-window-set",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-fzf]')\n"
                                        "print('|synthesis algorithm=query-set "
                                        "scope=text windowSet=owner:src/a.ts,"
                                        "tests:tests/a.test.ts,"
                                        "read:src/b.ts window_set=owner:src/c.rs,"
                                        "tests:tests/c_test.rs')"
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

    def test_line_protocol_rejects_window_set_rank_prefixed_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-window-set-rank-path",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-window-set-rank",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-fzf]')\n"
                                        "print('|synthesis algorithm=query-set "
                                        "scope=text windowSet=owner:0:src/a.ts')"
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

    def test_line_protocol_rejects_window_set_test_path_as_owner(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-window-set-owner-test-path",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-window-set-owner-test",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-fzf]')\n"
                                        "print('|synthesis algorithm=query-set "
                                        "scope=text windowSet=owner:tests/a.test.ts')"
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
            self.assertIn("owner target points at test path", result.steps[0].errors[0])

    def test_line_protocol_rejects_window_set_source_path_as_tests(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.bad-window-set-tests-source-path",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "bad-window-set-tests-source",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-fzf]')\n"
                                        "print('|synthesis algorithm=query-set "
                                        "scope=text windowSet=tests:src/a.ts')"
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
            self.assertIn("tests target is not test-like", result.steps[0].errors[0])


if __name__ == "__main__":
    unittest.main()
