"""Validate line-protocol rejection of rank-prefixed path fields."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolRankPrefixPathTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
