"""Receipt summary and output-mode count validation tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import main


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


class ReceiptCountValidationTests(unittest.TestCase):
    def test_receipt_cli_validates_summary_consistency(self) -> None:
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
                        "intent": "Explore async IO before editing",
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
                                },
                            }
                        ],
                        "summary": {
                            "commandCount": 2,
                            "stdoutBytes": 20,
                            "stderrBytes": 0,
                            "elapsedMs": 2,
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
        self.assertIn("[receipt] receipts=1 pass=0 fail=1", stdout.getvalue())
        self.assertIn("summary.commandCount=2 but commands=1", stdout.getvalue())

    def test_receipt_cli_validates_output_mode_counts(self) -> None:
        repo_root = _PROTOCOL_REPO_ROOT
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = Path(tmp) / "receipt.json"
            receipt_path.write_text(
                json.dumps(
                    {
                        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
                        "schemaVersion": "1",
                        "scenarioId": "typescript.receipt",
                        "language": "typescript",
                        "project": {"name": "typescript-lang-project-harness"},
                        "intent": "Explore parser tests",
                        "editBoundary": "before-edit",
                        "commands": [
                            {
                                "id": "text-json",
                                "kind": "search",
                                "argv": [
                                    "ts-harness",
                                    "search",
                                    "text",
                                    "projectRoot",
                                    "--json",
                                    ".",
                                ],
                                "metrics": {
                                    "elapsedMs": 2,
                                    "stdoutBytes": 20,
                                    "stderrBytes": 0,
                                },
                            }
                        ],
                        "summary": {
                            "commandCount": 1,
                            "stdoutBytes": 20,
                            "stderrBytes": 0,
                            "elapsedMs": 2,
                            "jsonSearches": 0,
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
        self.assertIn("summary.jsonSearches=0", stdout.getvalue())
        self.assertIn("outputMode=json search commands=1", stdout.getvalue())



if __name__ == "__main__":
    unittest.main()
