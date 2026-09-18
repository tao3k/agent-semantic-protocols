# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Guide-quality validation for the current Search Playbook route."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class RealTriggerGraphOutputGuideTests(unittest.TestCase):
    def test_guide_quality_accepts_search_playbook_route(self) -> None:
        output = "[search-playbook] language=typescript evidence=bounded"
        decision = {
            "reasonKind": "raw-broad-search",
            "languageIds": ["typescript"],
            "routes": [
                {
                    "kind": "playbook",
                    "argv": [
                        "asp",
                        "search",
                        "playbook",
                        "--language",
                        "typescript",
                    ],
                }
            ],
            "message": "Use the Search Playbook.",
        }
        result = self._run(
            {
                "agentHookDecision": decision,
                "searchOutput": output,
            },
            {
                "reasonKind": "raw-broad-search",
                "languageId": "typescript",
                "routeKind": "playbook",
                "routeCommandContains": ["asp search playbook"],
                "outputContains": [output],
            },
        )

        self.assertEqual("pass", result.status)

    def test_guide_quality_rejects_graph_selector_drift(self) -> None:
        bad_output = (
            "aliases=G:search,F:reasoning-selector\n"
            "F2=finding:finding(finding(serde))!finding"
        )
        result = self._run(
            {
                "agentHookDecision": {
                    "reasonKind": "raw-broad-search",
                    "languageIds": ["typescript"],
                    "routes": [],
                },
                "searchOutput": bad_output,
            },
            {
                "reasonKind": "raw-broad-search",
                "languageId": "typescript",
            },
        )

        self.assertEqual("fail", result.status)
        self.assertIn(
            "guide output contains graph drift text 'reasoning-selector'",
            result.steps[0].errors,
        )
        self.assertIn(
            "guide output contains graph drift text 'finding(finding('",
            result.steps[0].errors,
        )

    @staticmethod
    def _run(payload: dict[str, object], guide_quality: dict[str, object]):
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.search-playbook-guide",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    f"import json; print(json.dumps({payload!r}))",
                                ],
                                "expect": {"guideQuality": guide_quality},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            return run_scenario(repo_root, scenario_path)
