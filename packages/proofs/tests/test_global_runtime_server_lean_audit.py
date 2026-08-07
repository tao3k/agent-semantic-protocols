"""Verify the global Runtime Server Lean audit and digest-bound AXLE evidence."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


PROOFS_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PROOFS_ROOT.parents[1]
SCHEMAS_ROOT = REPOSITORY_ROOT / "schemas"
LEAN_SOURCE = PROOFS_ROOT / "ASPProof" / "GlobalRuntimeServer.lean"
AUDIT = PROOFS_ROOT / "receipts" / "global-runtime-server-audit-v1.json"
AXLE_RECEIPT = (
    PROOFS_ROOT / "receipts" / "global-runtime-server-axle-v1.json"
)
OBLIGATION = (
    PROOFS_ROOT / "receipts" / "global-runtime-server-obligation-v1.json"
)
PROOF_RECEIPT = (
    PROOFS_ROOT / "receipts" / "global-runtime-server-proof-receipt-v1.json"
)
CLI = [sys.executable, "-m", "asp_proofs", "lean-audit"]

REQUIRED_FAMILIES = [
    "global-server-state-home-identity",
    "no-project-server-identity",
    "ensure-idempotence",
    "concurrent-ensure-singleton",
    "graceful-stop-admission-fence",
    "graceful-stop-drain-checkpoint",
    "restart-assumption-convergence",
    "restart-assumption-counterexample",
    "typed-startup-failure",
    "internal-project-dispatch-isolation",
    "internal-project-dispatch-targeting",
    "provider-failure-global-lifecycle-isolation",
    "provider-failure-project-isolation",
    "fixed-hook-contract-preservation",
    "endpoint-hint-not-liveness",
    "socket-file-not-liveness",
    "authenticated-socket-liveness",
    "socket-liveness-handshake-necessity",
    "missing-socket-election",
    "refused-socket-election",
    "protocol-mismatch-fail-closed",
    "identity-mismatch-fail-closed",
    "operator-stop-precedence",
    "kernel-peer-uid-admission",
    "nonce-owner-epoch-single-use",
    "nonce-replay-fail-closed",
    "nonce-replay-no-mutation",
    "nonce-recorded-single-use",
    "recorded-nonce-replay-fail-closed",
    "nonce-capacity-exhaustion-no-eviction",
    "nonce-capacity-exhaustion-no-mutation",
    "nonce-capacity-bound",
    "runtime-directory-owner-symlink-security",
    "runtime-path-private-modes",
    "socket-resource-bounds",
    "frame-bound-fail-closed",
    "connection-bound-fail-closed",
    "hook-worker-bound-fail-closed",
    "drain-bound-fail-closed",
    "secret-redaction",
]

REQUIRED_RFC_CLAUSES = [
    "ASP-RFC-10.32-GLOBAL-SERVER-IDENTITY",
    "ASP-RFC-10.32-INTERNAL-PROJECT-DISPATCH",
    "ASP-RFC-10.32-ENSURE-IDEMPOTENCE",
    "ASP-RFC-10.32-GRACEFUL-STOP",
    "ASP-RFC-10.32-RESTART-CONVERGENCE",
    "ASP-RFC-10.32-TYPED-FAILURE",
    "ASP-RFC-10.32-FIXED-HOOK-CONTRACT",
    "ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY",
    "ASP-RFC-10.32-SOCKET-SECURITY-HARDENING",
]


def sha256(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def validator(schema_name: str) -> Draft202012Validator:
    registry = Registry()
    loaded: dict[str, object] = {}
    for name in [
        "semantic-proof-definitions.v1.schema.json",
        "semantic-proof-obligation.v1.schema.json",
        "semantic-proof-receipt.v1.schema.json",
    ]:
        schema = load_json(SCHEMAS_ROOT / name)
        loaded[name] = schema
        registry = registry.with_resource(
            str(schema["$id"]), Resource.from_contents(schema)
        )
    return Draft202012Validator(loaded[schema_name], registry=registry)


class GlobalRuntimeServerLeanAuditTest(unittest.TestCase):
    def run_audit(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [*CLI, str(AUDIT), *extra],
            cwd=REPOSITORY_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_axle_receipt_is_reproduced_and_digest_bound(self) -> None:
        required_arguments = [
            "--allow-axiom",
            "propext",
            "--allow-axiom",
            "Quot.sound",
        ]
        for family in REQUIRED_FAMILIES:
            required_arguments.extend(["--require-family", family])
        for clause in REQUIRED_RFC_CLAUSES:
            required_arguments.extend(["--require-rfc-clause", clause])

        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "receipt.json"
            completed = self.run_audit(
                *required_arguments, "--output", str(output)
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(load_json(output), load_json(AXLE_RECEIPT))
            self.assertEqual(
                "sha256:" + load_json(output)["auditSha256"], sha256(AUDIT)
            )

    def test_semantic_artifacts_validate_and_bind_all_inputs(self) -> None:
        obligation = load_json(OBLIGATION)
        receipt = load_json(PROOF_RECEIPT)

        self.assertEqual(
            list(validator("semantic-proof-obligation.v1.schema.json").iter_errors(obligation)),
            [],
        )
        self.assertEqual(
            list(validator("semantic-proof-receipt.v1.schema.json").iter_errors(receipt)),
            [],
        )
        self.assertEqual(receipt["obligationId"], obligation["obligationId"])
        self.assertEqual(receipt["formalStatementDigest"], sha256(LEAN_SOURCE))
        self.assertEqual(receipt["candidateDigest"], sha256(AXLE_RECEIPT))
        self.assertEqual(receipt["fields"]["leanAuditSha256"], sha256(AUDIT))
        self.assertFalse(load_json(AUDIT)["hasSorryAx"])

    def test_missing_required_family_is_rejected(self) -> None:
        completed = self.run_audit(
            "--allow-axiom",
            "propext",
            "--require-family",
            "not-a-global-runtime-server-family",
        )
        self.assertNotEqual(completed.returncode, 0)

    def test_unapproved_axiom_policy_is_rejected(self) -> None:
        completed = self.run_audit()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("unexpected axioms: Quot.sound, propext", completed.stderr)


if __name__ == "__main__":
    unittest.main()
