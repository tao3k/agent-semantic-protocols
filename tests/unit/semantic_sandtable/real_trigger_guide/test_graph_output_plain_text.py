"""Guide-quality plain text graph output tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class RealTriggerPlainTextGraphGuideTests(unittest.TestCase):
    def test_guide_quality_accepts_plain_text_output_without_decision(self) -> None:
        output = (
            "[guide] lang=rust provider=asp-rust protocol=guide.v1\n"
            "|catalog reasoningProfiles=owner-query,query-deps,owner-tests,finding-frontier,feature-cfg "
            "entries=owner-query,query-deps,owner-tests,finding-frontier,feature-cfg "
            "routes=path,read-frontier\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.plain-guide-quality",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    f"print({output!r}, end='')",
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "outputContains": ["|catalog reasoningProfiles="],
                                        "outputNotContains": ["profiles=", "owner-items"],
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

    def test_guide_quality_accepts_graph_alias_declaration_for_typed_entries(
        self,
    ) -> None:
        entries = "entries=owner-tests(O=>covering-tests+test-entrypoints+fixtures)"
        output = (
            "[search-prime] root=. alg=budgeted-prime-frontier-v1 budget=handles:12\n"
            "aliases: graph:{G=search,O=owner}\n"
            f"{entries}\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.graph-alias-entry",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "prime",
                                "command": [
                                    "python",
                                    "-c",
                                    f"print({output!r}, end='')",
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "primeOutput": {
                                            "requiresTypedEntryAliases": True,
                                            "entries": [entries],
                                        }
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
