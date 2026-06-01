"""Coverage-reporting behavior tests for semantic sandtables."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import main
from tools.semantic_sandtable.coverage import coverage_report


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


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

    def test_coverage_reports_large_library_intent_matrix(self) -> None:
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
                                "items": {"enum": ["large-library"]},
                            }
                        },
                    }
                ),
                encoding="utf-8",
            )
            for package in ["a", "b", "c"]:
                (repo_root / f"{package}.json").write_text(
                    json.dumps(
                        {
                            "id": f"python.{package}",
                            "language": "python",
                            "workdir": ".",
                            "coverage": ["large-library"],
                            "evidence": {
                                "source": "handwritten",
                                "fixtureTier": "large-library",
                                "targetLibrary": {
                                    "language": "python",
                                    "name": package,
                                    "package": package,
                                    "workdirKind": "checkout",
                                },
                                "intentCases": [
                                    {
                                        "intentKind": "feature-implementation",
                                        "intent": "feature",
                                        "stepIds": ["feature"],
                                    },
                                    {
                                        "intentKind": "api-usage",
                                        "intent": "api",
                                        "stepIds": ["api"],
                                    },
                                    {
                                        "intentKind": "implementation-principle",
                                        "intent": "principle",
                                        "stepIds": ["principle"],
                                    },
                                ],
                            },
                            "steps": [
                                {"id": "feature", "command": ["missing-binary"]},
                                {"id": "api", "command": ["missing-binary"]},
                                {"id": "principle", "command": ["missing-binary"]},
                            ],
                        }
                    ),
                    encoding="utf-8",
                )

            report = coverage_report(
                repo_root,
                [repo_root / "a.json", repo_root / "b.json", repo_root / "c.json"],
            )

        self.assertEqual([], report.large_library_missing.get("python", []))
        self.assertEqual(3, len(report.large_library_targets["python"]))

    def test_coverage_cli_can_fail_on_missing_large_library_matrix(self) -> None:
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
                                "items": {"enum": ["large-library"]},
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
                        "id": "python.large",
                        "language": "python",
                        "workdir": ".",
                        "coverage": ["large-library"],
                        "evidence": {
                            "source": "handwritten",
                            "fixtureTier": "large-library",
                            "targetLibrary": {
                                "language": "python",
                                "name": "demo",
                                "package": "demo",
                                "workdirKind": "checkout",
                            },
                            "intentCases": [
                                {
                                    "intentKind": "feature-implementation",
                                    "intent": "feature",
                                    "stepIds": ["feature"],
                                }
                            ],
                        },
                        "steps": [{"id": "feature", "command": ["missing-binary"]}],
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
                                "requiredCoverage": ["large-library"],
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

        output = stdout.getvalue()
        self.assertEqual(1, exit_code)
        self.assertIn("|intent-matrix language=python libraries=1/3 missing=2", output)
        self.assertIn("|missing language=python large-library=libraries=1/3", output)
        self.assertIn(
            "|missing language=python large-library=demo:intents=api-usage,implementation-principle",
            output,
        )



if __name__ == "__main__":
    unittest.main()
