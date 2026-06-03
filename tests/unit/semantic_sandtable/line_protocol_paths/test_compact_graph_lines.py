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
                                        "print('alias: graph:{G=search,O=owner,T=test}')\n"
                                        "print('G=search:result!query')\n"
                                        "print('O=owner:src/lib.rs!owner;T=test:tests/lib.rs!tests')\n"
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
