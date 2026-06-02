"""Coverage-reporting behavior tests for semantic sandtables."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.coverage import coverage_report


class CoverageReportTests(unittest.TestCase):
    def test_coverage_report_uses_schema_surfaces_and_step_coverage(self) -> None:
            with tempfile.TemporaryDirectory() as tmp:
                repo_root = Path(tmp)
                schema_dir = repo_root / "schemas"
                schema_dir.mkdir()
                (schema_dir / "semantic-sandtable-scenario.v1.schema.json").write_text(
                    json.dumps(
                        {
                            "$schema": "https://json-schema.org/draft/2020-12/schema",
                            "type": "object",
                            "required": ["id", "language", "workdir", "steps"],
                            "properties": {
                                "id": {"type": "string"},
                                "language": {"type": "string"},
                                "workdir": {"type": "string"},
                                "coverage": {
                                    "$ref": "#/$defs/coverageList",
                                },
                                "steps": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "string"},
                                            "command": {"type": "array"},
                                            "coverage": {
                                                "$ref": "#/$defs/coverageList",
                                            },
                                        },
                                    },
                                },
                            },
                            "$defs": {
                                "coverageList": {
                                    "type": "array",
                                    "items": {
                                        "enum": [
                                            "search-flow",
                                            "deps-query",
                                            "codex-hooks",
                                        ]
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
                            "id": "python.search",
                            "language": "python",
                            "workdir": ".",
                            "coverage": ["search-flow"],
                            "steps": [
                                {
                                    "id": "deps",
                                    "coverage": ["deps-query"],
                                    "command": ["missing-binary"],
                                }
                            ],
                        }
                    ),
                    encoding="utf-8",
                )

                report = coverage_report(repo_root, [scenario_path])

            self.assertEqual(1, report.scenario_count)
            self.assertEqual({"python"}, report.language_ids)
            self.assertIn("search-flow", report.surfaces)
            self.assertIn("deps-query", report.surfaces)
            self.assertEqual(["codex-hooks"], report.missing)
            self.assertEqual({"python.search:deps"}, report.surfaces["deps-query"].step_ids)

    def test_coverage_report_applies_per_language_policy(self) -> None:
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
                            "id": "python.search",
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

                report = coverage_report(repo_root, [scenario_path], policy_path=policy_path)

            self.assertEqual(["deps-query"], report.language_missing["python"])
            self.assertEqual({"search-flow"}, report.covered_surfaces_for_language("python"))


if __name__ == "__main__":
    unittest.main()
