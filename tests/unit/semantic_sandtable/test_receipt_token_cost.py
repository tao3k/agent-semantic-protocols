"""Receipt token-cost validation tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import main


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


class ReceiptTokenCostValidationTests(unittest.TestCase):
    def test_receipt_cli_validates_command_token_cost_total(self) -> None:
        repo_root = _PROTOCOL_REPO_ROOT
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = Path(tmp) / "receipt.json"
            receipt_path.write_text(
                json.dumps(
                    {
                        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
                        "schemaVersion": "1",
                        "scenarioId": "rust.receipt",
                        "language": "rust",
                        "project": {"name": "tokio"},
                        "intent": "Explore before editing",
                        "editBoundary": "before-edit",
                        "commands": [
                            {
                                "id": "prime",
                                "kind": "search",
                                "argv": ["rs-harness", "search", "prime", "."],
                                "metrics": {
                                    "elapsedMs": 2,
                                    "stdoutBytes": 20,
                                    "stderrBytes": 0,
                                    "tokenCost": {
                                        "unit": "token-estimate",
                                        "outputTokens": 5,
                                        "totalTokens": 5,
                                        "basis": "stdoutBytes/4 rounded up",
                                    },
                                },
                            },
                            {
                                "id": "owner",
                                "kind": "search",
                                "argv": [
                                    "rs-harness",
                                    "search",
                                    "owner",
                                    "src/lib.rs",
                                    ".",
                                ],
                                "metrics": {
                                    "elapsedMs": 2,
                                    "stdoutBytes": 20,
                                    "stderrBytes": 0,
                                },
                            },
                        ],
                        "summary": {
                            "commandCount": 2,
                            "stdoutBytes": 40,
                            "stderrBytes": 0,
                            "elapsedMs": 4,
                            "tokenCost": {
                                "unit": "token-estimate",
                                "outputTokens": 10,
                                "totalTokens": 10,
                                "basis": "sum of command estimates",
                            },
                        },
                    }
                ),
                encoding="utf-8",
            )
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(
                    ["--repo-root", str(repo_root), "--receipt", str(receipt_path)]
                )

        self.assertEqual(1, exit_code)
        self.assertIn("requires command metrics.tokenCost", stdout.getvalue())
        self.assertIn("owner", stdout.getvalue())




if __name__ == "__main__":
    unittest.main()
