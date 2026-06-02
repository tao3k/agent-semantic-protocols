"""Coverage-reporting behavior tests for semantic sandtables."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main


class CoverageCliTests(unittest.TestCase):
    def test_coverage_cli_does_not_execute_scenario_commands(self) -> None:
            with tempfile.TemporaryDirectory() as tmp:
                repo_root = Path(tmp)
                scenario_path = repo_root / "scenario.json"
                scenario_path.write_text(
                    json.dumps(
                        {
                            "id": "python.coverage",
                            "language": "python",
                            "workdir": ".",
                            "coverage": ["search-flow"],
                            "steps": [{"id": "never-run", "command": ["missing-binary"]}],
                        }
                    ),
                    encoding="utf-8",
                )
                stdout = StringIO()
                with redirect_stdout(stdout):
                    exit_code = main(
                        ["--coverage", "--repo-root", str(repo_root), "scenario.json"]
                    )

            self.assertEqual(0, exit_code)
            self.assertIn("[coverage]", stdout.getvalue())
            self.assertIn("|surface search-flow", stdout.getvalue())

    def test_coverage_cli_can_fail_on_missing_policy_surfaces(self) -> None:
            with tempfile.TemporaryDirectory() as tmp:
                repo_root = Path(tmp)
                schema_dir = repo_root / "schemas"
                schema_dir.mkdir()
                (schema_dir / "semantic-sandtable-scenario.v1.schema.json").write_text(
                    json.dumps(
                        {
                            "$schema": "https://json-schema.org/draft/2020-12/schema",
                            "type": "object",
                            "$defs": {
                                "coverageList": {
                                    "type": "array",
                                    "items": {
                                        "enum": ["search-flow", "deps-query"],
                                    },
                                }
                            },
                        }
                    ),
                    encoding="utf-8",
                )
                scenario_path = repo_root / "scenario.json"
                scenario_path.write_text(
                    json.dumps(
                        {
                            "id": "python.coverage",
                            "language": "python",
                            "workdir": ".",
                            "coverage": ["search-flow"],
                            "steps": [{"id": "never-run", "command": ["missing-binary"]}],
                        }
                    ),
                    encoding="utf-8",
                )
                policy_path = repo_root / "coverage-policy.json"
                policy_path.write_text(
                    json.dumps(
                        {
                            "schemaVersion": "semantic-sandtable-coverage-policy.v1",
                            "languages": [
                                {
                                    "languageId": "python",
                                    "requiredCoverage": ["search-flow", "deps-query"],
                                }
                            ],
                        }
                    ),
                    encoding="utf-8",
                )
                stdout = StringIO()
                with redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "--coverage",
                            "--fail-on-missing",
                            "--repo-root",
                            str(repo_root),
                            "--coverage-policy",
                            "coverage-policy.json",
                            "scenario.json",
                        ]
                    )

            self.assertEqual(1, exit_code)
            self.assertIn("|missing language=python surface=deps-query", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
