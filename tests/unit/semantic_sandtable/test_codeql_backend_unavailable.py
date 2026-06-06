"""ASP CodeQL unavailable sandtable coverage."""

from __future__ import annotations

import os
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


_REPO_ROOT = Path(__file__).resolve().parents[3]


class CodeqlBackendUnavailableSandtableTests(unittest.TestCase):
    def test_codeql_backend_unavailable_evidence_scenario_passes(self) -> None:
        result = run_scenario(
            _REPO_ROOT,
            _REPO_ROOT / "sandtables/rust/codeql-backend-unavailable-flow.json",
        )

        self.assertEqual("pass", result.status, result.errors)
        self.assertEqual(["pass"], [step.status for step in result.steps])

    def test_codeql_cli_metadata_evidence_scenario_passes(self) -> None:
        result = run_scenario(
            _REPO_ROOT,
            _REPO_ROOT / "sandtables/rust/codeql-cli-metadata-flow.json",
        )

        self.assertEqual("pass", result.status, result.errors)
        self.assertEqual(["pass"], [step.status for step in result.steps])

    def test_codeql_hot_path_native_search_scenario_passes(self) -> None:
        result = run_scenario(
            _REPO_ROOT,
            _REPO_ROOT / "sandtables/rust/codeql-hot-path-native-search-flow.json",
        )

        self.assertEqual("pass", result.status, result.errors)
        self.assertEqual(["pass"], [step.status for step in result.steps])

    @unittest.skipUnless(
        os.environ.get("ASP_RUN_SLOW_CODEQL") == "1",
        "slow CodeQL database/query evidence scenario requires ASP_RUN_SLOW_CODEQL=1",
    )
    def test_codeql_bounded_source_file_evidence_scenario_passes(self) -> None:
        result = run_scenario(
            _REPO_ROOT,
            _REPO_ROOT / "sandtables/rust/codeql-bounded-source-file-flow.json",
        )

        self.assertEqual("pass", result.status, result.errors)
        self.assertEqual(["pass"], [step.status for step in result.steps])


if __name__ == "__main__":
    unittest.main()
