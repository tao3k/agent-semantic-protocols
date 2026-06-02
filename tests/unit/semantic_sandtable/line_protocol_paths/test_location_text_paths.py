"""Validate line-protocol path handling for locations and text snippets."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolLocationTextPathTests(unittest.TestCase):
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
                                        "print('[search-fzf]')\n"
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
