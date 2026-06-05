"""Validate compact graph line protocol routing for sandtable evidence."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolCompactGraphTests(unittest.TestCase):
    def test_line_protocol_accepts_compact_graph_edges(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.compact-graph-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "compact-graph",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-prime] root=. alg=owner-rank-frontier')\n"
                                        "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                                        "print('alias: graph:{G=search,O=owner,T=test}')\n"
                                        "print('O=owner:path(src/lib.rs)!owner;T=test:path(tests/lib.rs)!tests')\n"
                                        "print('G>{O:selects,T:covers}')\n"
                                        "print('rank=O,T frontier=O.owner,T.tests')"
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

    def test_line_protocol_rejects_unclosed_compact_graph_alias_line(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.compact-graph-unclosed-alias",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "compact-graph",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-prime] root=. alg=owner-rank-frontier')\n"
                                        "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                                        "print('alias: graph:{G=search,O=owner')\n"
                                        "print('O=owner:path(src/lib.rs)!owner')\n"
                                        "print('G>{O:selects}')\n"
                                        "print('rank=O frontier=O.owner')"
                                    ),
                                    "search",
                                    "--view",
                                    "seeds",
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
            self.assertIn(
                "line protocol stray line: 'alias: graph:{G=search,O=owner'",
                result.steps[0].errors,
            )

    def test_line_protocol_accepts_compact_graph_omit_and_avoid(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.compact-graph-omit-avoid-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "compact-graph-omit-avoid",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-owner] owner=src/lib.rs alg=owner')\n"
                                        "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                                        "print('alias: graph:{G=search,O=owner}')\n"
                                        "print('O=owner:path(src/lib.rs)!owner')\n"
                                        "print('G>{O:selects}')\n"
                                        "print('rank=O frontier=O.owner')\n"
                                        "print('omit=code,comments,blank-lines,nonmatching-items')\n"
                                        "print('avoid=raw-read,source-dump')"
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

    def test_search_seeds_line_protocol_requires_compact_graph_block(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.compact-graph-required",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "missing-graph",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    "print('[search-prime] root=. alg=owner-rank-frontier')",
                                    "search",
                                    "--view",
                                    "seeds",
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
            self.assertIn(
                "compact graph missing micro-legend line",
                result.steps[0].errors,
            )

    def test_line_protocol_accepts_empty_compact_graph_block(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.compact-empty-graph-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "empty-graph",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "print('[search-prime] root=. alg=owner-rank-frontier')\n"
                                        "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                                        "print('alias: graph:{G=search}')\n"
                                        "print('G>{}')\n"
                                        "print('rank= frontier=')"
                                    ),
                                    "search",
                                    "--view",
                                    "seeds",
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
