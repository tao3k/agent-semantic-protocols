from __future__ import annotations

import copy

import pytest

from tools.axle_lean_audit import (
    LeanAuditContractError,
    LeanAuditPolicy,
    validate_lean_audit,
)


def audit_document() -> dict[str, object]:
    return {
        "schemaId": "asp.lean-proof-audit.v1",
        "schemaVersion": "1",
        "leanVersion": "4.32.2",
        "proofPackage": "ASPProof",
        "module": "ASPProof.SearchLoopTrace",
        "sourcePath": "packages/proofs/ASPProof/SearchLoopTrace.lean",
        "declarationCount": 1,
        "axiomFreeDeclarationCount": 0,
        "axiomDependentDeclarationCount": 1,
        "declarations": [
            {
                "name": "SearchLoopTrace.trace_preserves_domain",
                "kind": "theorem",
                "theoremFamily": "trace-safety",
                "rfcClauseIds": ["ASP-RFC-10.05-CFR-GOD-DECISION"],
                "type": "Trace initial final length → final.domain = initial.domain",
                "axioms": ["propext"],
                "hasSorryAx": False,
            }
        ],
        "axiomInventory": ["propext"],
        "hasSorryAx": False,
    }


def policy() -> LeanAuditPolicy:
    return LeanAuditPolicy(
        allowed_axioms=frozenset({"propext"}),
        required_theorem_families=frozenset({"trace-safety"}),
        required_rfc_clause_ids=frozenset({"ASP-RFC-10.05-CFR-GOD-DECISION"}),
    )


def test_valid_audit_produces_machine_receipt() -> None:
    receipt = validate_lean_audit(audit_document(), policy())

    assert receipt.declaration_count == 1
    assert receipt.axiom_free_declaration_count == 0
    assert receipt.axiom_dependent_declaration_count == 1
    assert receipt.theorem_families == ("trace-safety",)
    assert receipt.axiom_inventory == ("propext",)
    assert len(receipt.audit_sha256) == 64


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("schemaId", "wrong", "schemaId"),
        ("schemaVersion", "2", "schemaVersion"),
        ("declarationCount", 2, "declarationCount"),
        ("hasSorryAx", True, "hasSorryAx"),
    ],
)
def test_root_contract_mismatch_fails_closed(
    field: str, value: object, message: str
) -> None:
    document = audit_document()
    document[field] = value

    with pytest.raises(LeanAuditContractError, match=message):
        validate_lean_audit(document, policy())


def test_sorry_axiom_fails_closed() -> None:
    document = audit_document()
    declaration = document["declarations"][0]
    declaration["axioms"] = ["Lean.ofReduceBool", "sorryAx"]
    declaration["hasSorryAx"] = True
    document["axiomInventory"] = ["Lean.ofReduceBool", "sorryAx"]
    document["hasSorryAx"] = True

    with pytest.raises(LeanAuditContractError, match="sorryAx is forbidden"):
        validate_lean_audit(document, policy())


def test_axiom_union_mismatch_fails_closed() -> None:
    document = audit_document()
    document["axiomInventory"] = []

    with pytest.raises(LeanAuditContractError, match="axiomInventory"):
        validate_lean_audit(document, policy())


def test_unknown_axiom_fails_closed() -> None:
    document = audit_document()
    declaration = document["declarations"][0]
    declaration["axioms"] = ["Quot.sound"]
    document["axiomInventory"] = ["Quot.sound"]

    with pytest.raises(LeanAuditContractError, match="not admitted by policy"):
        validate_lean_audit(document, policy())


def test_repeated_family_with_distinct_theorem_names_is_valid() -> None:
    document = copy.deepcopy(audit_document())
    declarations = document["declarations"]
    assert isinstance(declarations, list)
    repeated = copy.deepcopy(declarations[0])
    repeated["name"] = "ASPProof.BoundEvidenceClause.second_family_obligation"
    declarations.append(repeated)
    document["declarationCount"] = len(declarations)
    if repeated["axioms"]:
        document["axiomDependentDeclarationCount"] += 1
    else:
        document["axiomFreeDeclarationCount"] += 1

    validate_lean_audit(document, policy())


def test_missing_family_and_clause_fail_closed() -> None:
    document = copy.deepcopy(audit_document())
    declaration = document["declarations"][0]
    declaration["theoremFamily"] = "bounded-progress"
    declaration["rfcClauseIds"] = ["ASP-RFC-10.05-BECA-STATES"]

    with pytest.raises(LeanAuditContractError, match="theorem families"):
        validate_lean_audit(document, policy())
