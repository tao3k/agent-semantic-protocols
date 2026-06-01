"""Runtime audit synthesis tests for executed sandtable results."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import main


class RuntimeAuditTests(unittest.TestCase):
    def test_text_report_summarizes_runtime_findings_and_actions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = _audit_fixture_repo(Path(tmp))
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(
                    [
                        "--repo-root",
                        str(repo_root),
                        "warn.json",
                        "fail.json",
                        "skip.json",
                    ]
                )

        output = stdout.getvalue()
        self.assertEqual(1, exit_code)
        self.assertIn("[sandtable-audit]", output)
        self.assertIn("kind=packet-size-budget", output)
        self.assertIn("kind=step-failure", output)
        self.assertIn("kind=large-library-skip", output)
        self.assertIn("kind=top-stdout-cost", output)
        self.assertIn("action=", output)

    def test_json_report_includes_runtime_audit_findings(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = _audit_fixture_repo(Path(tmp))
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(
                    [
                        "--json",
                        "--repo-root",
                        str(repo_root),
                        "warn.json",
                        "fail.json",
                        "skip.json",
                    ]
                )

        self.assertEqual(1, exit_code)
        payload = json.loads(stdout.getvalue())
        findings = payload["audit"]["findings"]
        self.assertGreaterEqual(payload["audit"]["summary"]["total"], 3)
        self.assertIn("packet-size-budget", {finding["kind"] for finding in findings})
        self.assertIn("step-failure", {finding["kind"] for finding in findings})
        self.assertIn("large-library-skip", {finding["kind"] for finding in findings})


def _audit_fixture_repo(root: Path) -> Path:
    schema_dir = root / "schemas"
    schema_dir.mkdir()
    (schema_dir / "semantic-sandtable-scenario.v1.schema.json").write_text(
        json.dumps({"$schema": "https://json-schema.org/draft/2020-12/schema"}),
        encoding="utf-8",
    )
    (root / "warn.json").write_text(
        json.dumps(
            {
                "id": "python.warn",
                "language": "python",
                "workdir": ".",
                "steps": [
                    {
                        "id": "noisy",
                        "command": [
                            sys.executable,
                            "-c",
                            "print('[ok]')\nprint('|owner src/a.py')",
                        ],
                        "expect": {
                            "lineProtocol": True,
                            "maxStdoutLinesWarn": 1,
                        },
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    (root / "fail.json").write_text(
        json.dumps(
            {
                "id": "python.fail",
                "language": "python",
                "workdir": ".",
                "steps": [
                    {
                        "id": "bad",
                        "command": [sys.executable, "-c", "import sys; sys.exit(3)"],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    (root / "skip.json").write_text(
        json.dumps(
            {
                "id": "python.large-skip",
                "language": "python",
                "coverage": ["large-library"],
                "workdir": {"candidates": ["missing-large-library"]},
                "evidence": {
                    "source": "handwritten",
                    "fixtureTier": "large-library",
                },
                "steps": [
                    {
                        "id": "never",
                        "command": [sys.executable, "-c", "print('[never]')"],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    return root


if __name__ == "__main__":
    unittest.main()
