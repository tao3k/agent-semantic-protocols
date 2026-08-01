"""Consume a Lean JSON audit and emit a compact AXLE admission receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys

__all__ = ["AuditAdmissionError", "admit_audit"]
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
LEAN_AUDIT_SCHEMA = REPOSITORY_ROOT / "schemas/lean-proof-audit.v1.schema.json"
AXLE_RECEIPT_SCHEMA = REPOSITORY_ROOT / "schemas/axle-lean-audit-receipt.v1.schema.json"


class AuditAdmissionError(ValueError):
    """Raised when a Lean audit cannot be admitted by AXLE policy."""


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AuditAdmissionError(f"expected a JSON object: {path}")
    return value


def _validate(instance: dict[str, Any], schema_path: Path) -> None:
    schema = _load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(instance),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        rendered = "; ".join(error.message for error in errors)
        raise AuditAdmissionError(f"schema validation failed: {rendered}")


def admit_audit(
    audit_path: Path,
    *,
    allowed_axioms: set[str],
    required_families: set[str],
    required_rfc_clauses: set[str],
) -> dict[str, Any]:
    audit_bytes = audit_path.read_bytes()
    audit = json.loads(audit_bytes)
    if not isinstance(audit, dict):
        raise AuditAdmissionError("Lean audit must be a JSON object")
    _validate(audit, LEAN_AUDIT_SCHEMA)

    declarations = audit["declarations"]
    theorem_families = {
        declaration["theoremFamily"] for declaration in declarations
    }
    rfc_clause_ids = {
        clause
        for declaration in declarations
        for clause in declaration["rfcClauseIds"]
    }
    observed_axioms = set(audit["axiomInventory"])

    unexpected_axioms = observed_axioms - allowed_axioms
    if unexpected_axioms:
        raise AuditAdmissionError(
            "unexpected axioms: " + ", ".join(sorted(unexpected_axioms))
        )
    missing_families = required_families - theorem_families
    if missing_families:
        raise AuditAdmissionError(
            "missing theorem families: " + ", ".join(sorted(missing_families))
        )
    missing_clauses = required_rfc_clauses - rfc_clause_ids
    if missing_clauses:
        raise AuditAdmissionError(
            "missing RFC clauses: " + ", ".join(sorted(missing_clauses))
        )
    if audit["hasSorryAx"]:
        raise AuditAdmissionError("Lean audit contains sorryAx")

    receipt = {
        "schemaId": "asp.axle-lean-audit-receipt.v1",
        "schemaVersion": "1",
        "auditSha256": hashlib.sha256(audit_bytes).hexdigest(),
        "declarationCount": audit["declarationCount"],
        "axiomFreeDeclarationCount": audit["axiomFreeDeclarationCount"],
        "axiomDependentDeclarationCount": audit["axiomDependentDeclarationCount"],
        "theoremFamilies": sorted(theorem_families),
        "rfcClauseIds": sorted(rfc_clause_ids),
        "axiomInventory": sorted(observed_axioms),
    }
    _validate(receipt, AXLE_RECEIPT_SCHEMA)
    return receipt


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Admit a Lean proof audit and emit an AXLE receipt."
    )
    parser.add_argument("audit", type=Path)
    parser.add_argument("--allow-axiom", action="append", default=[])
    parser.add_argument("--require-family", action="append", default=[])
    parser.add_argument("--require-rfc-clause", action="append", default=[])
    parser.add_argument("--output", type=Path)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        receipt = admit_audit(
            args.audit,
            allowed_axioms=set(args.allow_axiom),
            required_families=set(args.require_family),
            required_rfc_clauses=set(args.require_rfc_clause),
        )
    except (AuditAdmissionError, OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"AXLE audit rejected: {error}") from error

    rendered = json.dumps(receipt, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        sys.stdout.write(rendered)
    else:
        args.output.write_text(rendered, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
