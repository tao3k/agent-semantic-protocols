"""Validate sandtable JSON report projection contracts."""

from __future__ import annotations

import unittest
from pathlib import Path

from tools.semantic_sandtable.json_reports import report_json
from tools.semantic_sandtable.models import ScenarioResult


class JsonReportsTests(unittest.TestCase):
    def test_report_json_uses_repo_relative_scenario_paths(self) -> None:
        repo_root = Path("/repo")
        result = ScenarioResult(
            scenario_id="root.provider-preflight-smoke",
            language="root",
            path=repo_root / ".cache/agent-semantic-protocol/provider-preflight.json",
            status="pass",
            workdir=repo_root,
        )

        packet = report_json([result], repo_root)
        scenario = packet["scenarios"][0]

        self.assertEqual(
            ".cache/agent-semantic-protocol/provider-preflight.json",
            scenario["path"],
        )
        self.assertEqual(".", scenario["workdir"])
        self.assertNotIn("/repo", str(packet))

    def test_report_json_keeps_legacy_path_projection_without_repo_root(self) -> None:
        result = ScenarioResult(
            scenario_id="root.legacy",
            language="root",
            path=Path("/repo/sandtables/legacy.json"),
            status="pass",
            workdir=Path("/repo"),
        )

        scenario = report_json([result])["scenarios"][0]

        self.assertEqual("/repo/sandtables/legacy.json", scenario["path"])
        self.assertEqual("/repo", scenario["workdir"])
