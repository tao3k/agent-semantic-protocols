"""Receipt query-set opportunity reporting tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import main


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


class ReceiptQuerySetReportTests(unittest.TestCase):
    def test_receipt_query_set_opportunity_supports_terms_and_scope(self) -> None:
        repo_root = _PROTOCOL_REPO_ROOT
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = Path(tmp) / "receipt.json"
            receipt_path.write_text(
                json.dumps(
                    {
                        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
                        "schemaVersion": "1",
                        "scenarioId": "typescript.render-location-queryset",
                        "language": "typescript",
                        "project": {
                            "name": "typescript-lang-project-harness",
                            "source": "checkout",
                        },
                        "intent": "Repair semantic-search render location formatting",
                        "editBoundary": "after-search",
                        "commands": [
                            {
                                "id": "text-location-path",
                                "kind": "search",
                                "argv": [
                                    "ts-harness",
                                    "search",
                                    "text",
                                    "location.path",
                                    "owner",
                                    "tests",
                                    "--json",
                                    ".",
                                ],
                                "outputMode": "json",
                                "metrics": {
                                    "elapsedMs": 1,
                                    "stdoutBytes": 10,
                                    "stderrBytes": 0,
                                    "tokenCost": {
                                        "unit": "token-estimate",
                                        "outputTokens": 3,
                                        "totalTokens": 3,
                                        "basis": "stdoutBytes/4 rounded up",
                                    },
                                },
                            }
                        ],
                        "summary": {
                            "commandCount": 1,
                            "stdoutBytes": 10,
                            "stderrBytes": 0,
                            "elapsedMs": 1,
                            "repeatedSearches": 1,
                            "jsonSearches": 1,
                            "compactSearches": 0,
                            "tokenCost": {
                                "unit": "token-estimate",
                                "outputTokens": 3,
                                "totalTokens": 3,
                                "basis": "stdoutBytes/4 rounded up",
                            },
                        },
                        "querySetOpportunities": [
                            {
                                "view": "text",
                                "queries": 3,
                                "saveCommands": 2,
                                "selector": "exact-set",
                                "terms": [
                                    "location.path",
                                    "location.column",
                                    "location.line",
                                ],
                                "scope": {
                                    "ownerPath": "src/cli/semantic-search/render.ts"
                                },
                                "beforeCommandIds": ["text-location-path"],
                                "recommendedCommand": [
                                    "ts-harness",
                                    "search",
                                    "text",
                                    "--query-set",
                                    "location.path",
                                    "--owner",
                                    "src/cli/semantic-search/render.ts",
                                    ".",
                                ],
                                "reason": "same owner-scoped text axis",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(
                    ["--repo-root", str(repo_root), "--receipt", str(receipt_path)]
                )

        self.assertEqual(0, exit_code)
        self.assertIn("[receipt] receipts=1 pass=1 fail=0", stdout.getvalue())
        self.assertIn("jsonSearches=1 compactSearches=0", stdout.getvalue())
        self.assertIn("|tokenCost totalTokens=3", stdout.getvalue())
        self.assertIn(
            "|commandTokenCost id=text-location-path totalTokens=3",
            stdout.getvalue(),
        )
        self.assertIn("unit=token-estimate", stdout.getvalue())
        self.assertIn("|merge view=text queries=3", stdout.getvalue())
        self.assertIn("owner=src/cli/semantic-search/render.ts", stdout.getvalue())



if __name__ == "__main__":
    unittest.main()
