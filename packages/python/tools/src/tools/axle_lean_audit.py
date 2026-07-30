"""Fail-closed AXLE consumer for Lean-owned proof audit JSON."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .console import emit

SCHEMA_ID = "asp.lean-proof-audit.v1"
SCHEMA_VERSION = "1"


class LeanAuditContractError(ValueError):
    """The Lean audit is missing identity or violates the audit contract."""


@dataclass(frozen=True)
class LeanAuditPolicy:
    allowed_axioms: frozenset[str]
    required_theorem_families: frozenset[str] = frozenset()
    required_rfc_clause_ids: frozenset[str] = frozenset()


@dataclass(frozen=True)
class LeanAuditReceipt:
    audit_sha256: str
    declaration_count: int
    axiom_free_declaration_count: int
    axiom_dependent_declaration_count: int
    theorem_families: tuple[str, ...]
    rfc_clause_ids: tuple[str, ...]
    axiom_inventory: tuple[str, ...]

    def as_json(self) -> dict[str, object]:
        return {
            "schemaId": "asp.axle-lean-audit-receipt.v1",
            "schemaVersion": "1",
            "auditSha256": self.audit_sha256,
            "declarationCount": self.declaration_count,
            "axiomFreeDeclarationCount": self.axiom_free_declaration_count,
            "axiomDependentDeclarationCount": self.axiom_dependent_declaration_count,
            "theoremFamilies": list(self.theorem_families),
            "rfcClauseIds": list(self.rfc_clause_ids),
            "axiomInventory": list(self.axiom_inventory),
        }


def _mapping(value: object, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise LeanAuditContractError(f"{field} must be an object")
    return value


def _string(value: object, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise LeanAuditContractError(f"{field} must be a non-empty string")
    return value


def _string_list(value: object, field: str) -> list[str]:
    if not isinstance(value, list):
        raise LeanAuditContractError(f"{field} must be an array")
    result = [_string(item, f"{field}[]") for item in value]
    if result != sorted(set(result)):
        raise LeanAuditContractError(f"{field} must be sorted and unique")
    return result


def _bool(value: object, field: str) -> bool:
    if not isinstance(value, bool):
        raise LeanAuditContractError(f"{field} must be a boolean")
    return value


def _declarations(document: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    declarations = document.get("declarations")
    if not isinstance(declarations, list) or not declarations:
        raise LeanAuditContractError("declarations must be a non-empty array")
    return [
        _mapping(declaration, f"declarations[{index}]")
        for index, declaration in enumerate(declarations)
    ]


def validate_lean_audit(
    document: Mapping[str, Any],
    policy: LeanAuditPolicy,
) -> LeanAuditReceipt:
    if document.get("schemaId") != SCHEMA_ID:
        raise LeanAuditContractError(f"schemaId must be {SCHEMA_ID}")
    if document.get("schemaVersion") != SCHEMA_VERSION:
        raise LeanAuditContractError(f"schemaVersion must be {SCHEMA_VERSION}")

    _string(document.get("leanVersion"), "leanVersion")
    _string(document.get("proofPackage"), "proofPackage")
    _string(document.get("module"), "module")
    _string(document.get("sourcePath"), "sourcePath")

    declarations = _declarations(document)
    if document.get("declarationCount") != len(declarations):
        raise LeanAuditContractError("declarationCount does not match declarations")

    declaration_names: set[str] = set()
    theorem_families: set[str] = set()
    rfc_clause_ids: set[str] = set()
    declaration_axioms: set[str] = set()
    any_sorry_ax = False
    axiom_free_count = 0

    for index, declaration in enumerate(declarations):
        field = f"declarations[{index}]"
        name = _string(declaration.get("name"), f"{field}.name")
        if name in declaration_names:
            raise LeanAuditContractError(f"duplicate declaration name: {name}")
        declaration_names.add(name)

        if declaration.get("kind") != "theorem":
            raise LeanAuditContractError(f"{field}.kind must be theorem")
        theorem_families.add(
            _string(declaration.get("theoremFamily"), f"{field}.theoremFamily")
        )
        _string(declaration.get("type"), f"{field}.type")

        clause_ids = _string_list(
            declaration.get("rfcClauseIds"), f"{field}.rfcClauseIds"
        )
        if not clause_ids:
            raise LeanAuditContractError(f"{field}.rfcClauseIds must not be empty")
        if any(not clause_id.startswith("ASP-RFC-") for clause_id in clause_ids):
            raise LeanAuditContractError(f"{field}.rfcClauseIds contains an invalid id")
        rfc_clause_ids.update(clause_ids)

        axioms = _string_list(declaration.get("axioms"), f"{field}.axioms")
        if not axioms:
            axiom_free_count += 1
        declaration_axioms.update(axioms)
        has_sorry_ax = _bool(declaration.get("hasSorryAx"), f"{field}.hasSorryAx")
        if has_sorry_ax != any("sorryAx" in axiom for axiom in axioms):
            raise LeanAuditContractError(f"{field}.hasSorryAx disagrees with axioms")
        any_sorry_ax = any_sorry_ax or has_sorry_ax

    axiom_inventory = _string_list(document.get("axiomInventory"), "axiomInventory")
    if axiom_inventory != sorted(declaration_axioms):
        raise LeanAuditContractError(
            "axiomInventory does not equal the declaration axiom union"
        )
    root_has_sorry_ax = _bool(document.get("hasSorryAx"), "hasSorryAx")
    if root_has_sorry_ax != any_sorry_ax:
        raise LeanAuditContractError("root hasSorryAx disagrees with declarations")
    if root_has_sorry_ax:
        raise LeanAuditContractError("sorryAx is forbidden")
    if document.get("axiomFreeDeclarationCount") != axiom_free_count:
        raise LeanAuditContractError(
            "axiomFreeDeclarationCount does not match declarations"
        )
    axiom_dependent_count = len(declarations) - axiom_free_count
    if document.get("axiomDependentDeclarationCount") != axiom_dependent_count:
        raise LeanAuditContractError(
            "axiomDependentDeclarationCount does not match declarations"
        )

    unknown_axioms = declaration_axioms - policy.allowed_axioms
    if unknown_axioms:
        raise LeanAuditContractError(
            f"axioms are not admitted by policy: {sorted(unknown_axioms)}"
        )
    missing_families = policy.required_theorem_families - theorem_families
    if missing_families:
        raise LeanAuditContractError(
            f"required theorem families are missing: {sorted(missing_families)}"
        )
    missing_clause_ids = policy.required_rfc_clause_ids - rfc_clause_ids
    if missing_clause_ids:
        raise LeanAuditContractError(
            f"required RFC clause ids are missing: {sorted(missing_clause_ids)}"
        )

    canonical = json.dumps(document, sort_keys=True, separators=(",", ":")).encode()
    return LeanAuditReceipt(
        audit_sha256=hashlib.sha256(canonical).hexdigest(),
        declaration_count=len(declarations),
        axiom_free_declaration_count=axiom_free_count,
        axiom_dependent_declaration_count=axiom_dependent_count,
        theorem_families=tuple(sorted(theorem_families)),
        rfc_clause_ids=tuple(sorted(rfc_clause_ids)),
        axiom_inventory=tuple(axiom_inventory),
    )


def load_and_validate_lean_audit(
    path: Path,
    policy: LeanAuditPolicy,
) -> LeanAuditReceipt:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise LeanAuditContractError(f"cannot read Lean audit {path}: {error}") from error
    return validate_lean_audit(_mapping(document, "audit"), policy)


def _values(values: Iterable[str] | None) -> frozenset[str]:
    return frozenset(values or ())


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("audit", type=Path)
    parser.add_argument("--allow-axiom", action="append")
    parser.add_argument("--require-family", action="append")
    parser.add_argument("--require-rfc-clause", action="append")
    arguments = parser.parse_args(argv)
    policy = LeanAuditPolicy(
        allowed_axioms=_values(arguments.allow_axiom),
        required_theorem_families=_values(arguments.require_family),
        required_rfc_clause_ids=_values(arguments.require_rfc_clause),
    )
    try:
        receipt = load_and_validate_lean_audit(arguments.audit, policy)
    except LeanAuditContractError as error:
        parser.exit(2, f"axle lean audit: {error}\n")
    emit(json.dumps(receipt.as_json(), sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
