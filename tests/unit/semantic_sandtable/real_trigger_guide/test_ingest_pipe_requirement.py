"""Real-trigger evidence and guide-quality behavior tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[4]


class RealTriggerIngestPipeGuideTests(unittest.TestCase):
    def test_guide_quality_requires_ingest_pipe_when_declared(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.bad-guide",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "import json; "
                                        "print(json.dumps({"
                                        "'agentHookDecision': {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['rust'],"
                                        "'routes': [{"
                                        "'kind': 'owner',"
                                        "'argv': ['rs-harness', 'search', 'owner', 'src/lib.rs', '.']"
                                        "}],"
                                        "'message': 'Use owner search.'"
                                        "}}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "rust",
                                        "routeKind": "ingest",
                                        "requiresIngestPipe": True,
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn("guide missing route kind 'ingest'", result.steps[0].errors)
        self.assertIn("guide missing ingest pipe route", result.steps[0].errors)
