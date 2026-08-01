"""Verify the proof-owned Lean audit admission and AXLE receipt boundary."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


PROOFS_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PROOFS_ROOT.parents[1]
AUDIT = PROOFS_ROOT / "receipts" / (
    "agent-session-host-registry-semantic-abstraction-"
    "discriminator-conformance-audit-v1.json"
)
EXPECTED_RECEIPT = PROOFS_ROOT / "receipts" / (
    "agent-session-host-registry-semantic-abstraction-"
    "discriminator-conformance-axle-audit-v1.json"
)
CLI = [sys.executable, "-m", "asp_proofs", "lean-audit"]

REQUIRED_ARGUMENTS = [
    "--require-family",
    "abstraction-adequacy",
    "--require-family",
    "conformance-counterexamples",
    "--require-family",
    "discriminator-conformance",
    "--require-family",
    "encoding-injectivity",
    "--require-family",
    "replay-binding",
    "--require-rfc-clause",
    "ASP-RFC-10.05-ASAD-ABSTRACTION",
    "--require-rfc-clause",
    "ASP-RFC-10.05-ASAD-DISCRIMINATOR",
    "--require-rfc-clause",
    "ASP-RFC-10.05-ASAD-ENCODING",
    "--require-rfc-clause",
    "ASP-RFC-10.05-ASAD-REPLAY",
]


class AxleLeanAuditTest(unittest.TestCase):
    def run_audit(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [*CLI, str(AUDIT), *extra],
            cwd=REPOSITORY_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_existing_receipt_is_reproduced_semantically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "receipt.json"
            completed = self.run_audit(*REQUIRED_ARGUMENTS, "--output", str(output))

            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                json.loads(output.read_text()),
                json.loads(EXPECTED_RECEIPT.read_text()),
            )
            self.assertEqual(
                json.loads(output.read_text())["auditSha256"],
                hashlib.sha256(AUDIT.read_bytes()).hexdigest(),
            )

    def test_missing_required_family_is_rejected(self) -> None:
        completed = self.run_audit(
            "--require-family", "not-a-real-proof-family"
        )

        self.assertNotEqual(completed.returncode, 0)

    def test_missing_required_rfc_clause_is_rejected(self) -> None:
        completed = self.run_audit(
            "--require-rfc-clause", "ASP-RFC-NOT-A-REAL-CLAUSE"
        )

        self.assertNotEqual(completed.returncode, 0)


if __name__ == "__main__":
    unittest.main()
