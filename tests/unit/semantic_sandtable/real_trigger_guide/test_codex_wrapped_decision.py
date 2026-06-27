"""Real-trigger evidence and guide-quality behavior tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[4]


class RealTriggerCodexWrappedGuideTests(unittest.TestCase):
    def test_guide_quality_accepts_codex_wrapped_decision(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.codex-guide",
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
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['rust'],"
                                        "'routes': [{"
                                        "'kind': 'ingest',"
                                        "'argv': ['rs-harness', 'search', 'ingest', 'items', 'tests', '--view', 'seeds', '.']"
                                        "}],"
                                        "'message': 'Use rs-harness search ingest.'"
                                        "}; "
                                        "print(json.dumps({"
                                        "'hookSpecificOutput': {"
                                        "'additionalContext': '[agent-hook-decision] ' + json.dumps(decision),"
                                        "'permissionDecision': 'deny'"
                                        "}}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "rust",
                                        "routeKind": "ingest",
                                        "commandContains": [
                                            "rs-harness",
                                            "search",
                                            "ingest",
                                        ],
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

        self.assertEqual("pass", result.status)

    def test_guide_quality_rejects_stale_route_command(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.codex-guide",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "import json; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{"
                                        "'kind': 'query',"
                                        "'argv': ["
                                        "'ts-harness', 'search', 'query',"
                                        "'--from-hook', 'direct-source-read',"
                                        "'--selector', '**/*.ts',"
                                        "'--term', 'GuideQuality',"
                                        "'--surface', 'owner,tests',"
                                        "'--view', 'seeds', '.'"
                                        "]"
                                        "}],"
                                        "'message': 'Use ts-harness query --from-hook direct-source-read.'"
                                        "}; "
                                        "print(json.dumps({"
                                        "'hookSpecificOutput': {"
                                        "'additionalContext': '[agent-hook-decision] ' + json.dumps(decision),"
                                        "'permissionDecision': 'deny'"
                                        "}}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "routeCommandContains": [
                                            "ts-harness query --from-hook",
                                            "--surface owners,tests",
                                        ],
                                        "routeCommandNotContains": [
                                            "search query",
                                            "--surface owner,tests",
                                        ],
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
        self.assertTrue(
            any(
                "guide missing route command text 'ts-harness query --from-hook'"
                in error
                for step in result.steps
                for error in step.errors
            )
        )
        self.assertTrue(
            any(
                "guide route contains stale command text 'search query'" in error
                for step in result.steps
                for error in step.errors
            )
        )
