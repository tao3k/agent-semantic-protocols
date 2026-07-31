#!/usr/bin/env python3
"""Validate a Lean proof-audit receipt and emit a compact AXLE receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


LEAN_AUDIT_SCHEMA = "asp.lean-proof-audit.v1"
AXLE_AUDIT_SCHEMA = "asp.axle-lean-audit-receipt.v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("audit", type=Path)
    parser.add_argument("--allow-axiom", action="append", default=[])
    parser.add_argument("--require-clause", action="append", default=[])
    parser.add_argument("--require-family", action="append", default=[])
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def load_audit(path: Path) -> tuple[bytes, dict[str, Any]]:
    payload = path.read_bytes()
    parsed = json.loads(payload)
    require(isinstance(parsed, dict), "Lean audit must be a JSON object")
    return payload, parsed


def validate(args: argparse.Namespace) -> dict[str, Any]:
    _, audit = load_audit(args.audit)
    declarations = audit.get("declarations")
    require(audit.get("schemaId") == LEAN_AUDIT_SCHEMA, "unexpected Lean audit schema")
    require(audit.get("schemaVersion") == "1", "unexpected Lean audit schema version")
    require(audit.get("hasSorryAx") is False, "Lean audit reports sorryAx")
    require(isinstance(declarations, list), "declarations must be an array")
    require(
        audit.get("declarationCount") == len(declarations),
        "declaration count does not match declarations",
    )

    names = [item.get("name") for item in declarations]
    families = [item.get("theoremFamily") for item in declarations]
    theorem_types = [item.get("type") for item in declarations]
    require(all(isinstance(name, str) and name for name in names), "invalid theorem name")
    require(
        all(isinstance(family, str) and family for family in families),
        "invalid theorem family",
    )
    require(
        all(isinstance(theorem_type, str) and theorem_type for theorem_type in theorem_types),
        "invalid theorem type",
    )
    require(len(names) == len(set(names)), "duplicate theorem name")
    require(
        all(item.get("hasSorryAx") is False for item in declarations),
        "a declaration reports sorryAx",
    )

    require(
        all(isinstance(item.get("axioms"), list) for item in declarations),
        "declaration axioms must be arrays",
    )
    require(
        all(isinstance(item.get("rfcClauseIds"), list) for item in declarations),
        "declaration RFC clause IDs must be arrays",
    )
    axiom_sets = [set(item["axioms"]) for item in declarations]
    axiom_inventory = set().union(*axiom_sets) if axiom_sets else set()
    allowed_axioms = set(args.allow_axiom)
    require(
        axiom_inventory <= allowed_axioms,
        "axiom inventory contains a disallowed axiom",
    )
    require(
        set(audit.get("axiomInventory", [])) == axiom_inventory,
        "top-level axiom inventory does not match declarations",
    )

    dependent_count = sum(bool(axioms) for axioms in axiom_sets)
    require(
        audit.get("axiomDependentDeclarationCount") == dependent_count,
        "axiom-dependent declaration count mismatch",
    )
    require(
        audit.get("axiomFreeDeclarationCount") == len(declarations) - dependent_count,
        "axiom-free declaration count mismatch",
    )

    clause_ids = {
        clause
        for item in declarations
        for clause in item.get("rfcClauseIds", [])
        if isinstance(clause, str)
    }
    required_families = set(args.require_family)
    required_clauses = set(args.require_clause)
    if required_families:
        require(
            required_families <= set(families),
            "missing required theorem family",
        )
    if required_clauses:
        require(required_clauses <= clause_ids, "missing required RFC clause")

    return {
        "auditSha256": hashlib.sha256(
            json.dumps(audit, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest(),
        "axiomDependentDeclarationCount": dependent_count,
        "axiomFreeDeclarationCount": len(declarations) - dependent_count,
        "axiomInventory": sorted(axiom_inventory),
        "declarationCount": len(declarations),
        "rfcClauseIds": sorted(clause_ids),
        "schemaId": AXLE_AUDIT_SCHEMA,
        "schemaVersion": "1",
        "theoremFamilies": sorted(set(families)),
    }


def main() -> None:
    json.dump(
        validate(parse_args()),
        sys.stdout,
        separators=(",", ":"),
        sort_keys=True,
    )
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
