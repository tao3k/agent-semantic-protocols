# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Cross-family reference decision lifecycle tests."""

from __future__ import annotations

import json
from pathlib import Path

from asp_schema_manager.reference_decisions import (
    decision_identity,
    load_reference_decisions,
    reconcile_reference_decisions,
)


def _identity() -> dict:
    return {
        "fingerprint": "sha256:" + "a" * 64,
        "occurrenceSetDigest": "sha256:" + "b" * 64,
        "familyIds": ["asp.schema-family.a", "asp.schema-family.b"],
    }


def _opportunity() -> dict:
    value = _identity()
    value.pop("occurrenceSetDigest")
    return {
        **value,
        "occurrences": [
            {"schemaPath": "schemas/a.v1.schema.json", "jsonPointer": "/$defs/value"},
            {"schemaPath": "schemas/b.v1.schema.json", "jsonPointer": "/$defs/value"},
        ],
        "familyScope": "cross-family",
    }


def _entry() -> dict:
    entry = {
        **_identity(),
        "decision": "defer",
        "reviewState": "accepted",
        "owner": "schemas",
        "rationale": "Wait for a shared semantic owner.",
    }
    entry["occurrenceSetDigest"] = decision_identity(_opportunity())[1]
    return entry


def test_accepted_exact_decision_reconciles() -> None:
    assert reconcile_reference_decisions([_opportunity()], [_entry()]) == []


def test_changed_occurrence_reopens_decision() -> None:
    entry = _entry()
    entry["occurrenceSetDigest"] = "sha256:" + "c" * 64
    diagnostics = reconcile_reference_decisions([_opportunity()], [entry])
    assert {item["code"] for item in diagnostics} == {
        "cross-family-reference-decision-missing",
        "stale-reference-decision",
    }


def test_occurrence_digest_is_order_independent_and_drift_sensitive() -> None:
    opportunity = _opportunity()
    reversed_opportunity = {
        **opportunity,
        "occurrences": list(reversed(opportunity["occurrences"])),
    }
    changed_opportunity = {
        **opportunity,
        "occurrences": [
            *opportunity["occurrences"][:-1],
            {
                **opportunity["occurrences"][-1],
                "jsonPointer": "/$defs/changed",
            },
        ],
    }
    assert decision_identity(opportunity) == decision_identity(reversed_opportunity)
    assert decision_identity(opportunity) != decision_identity(changed_opportunity)


def test_duplicate_decision_identity_is_rejected(tmp_path: Path) -> None:
    registry_path = tmp_path / "decisions.json"
    registry_path.write_text(
        json.dumps(
            {
                "schemaId": "asp.schema-reference-decision-registry.v1",
                "schemaVersion": "1",
                "entries": [_entry(), _entry()],
            }
        ),
        encoding="utf-8",
    )
    _, diagnostics = load_reference_decisions(registry_path)
    assert any(
        item["code"] == "duplicate-reference-decision-entry"
        for item in diagnostics
    )


def test_registry_is_validated_by_contract(tmp_path: Path) -> None:
    registry_path = tmp_path / "decisions.json"
    registry_path.write_text(
        json.dumps(
            {
                "schemaId": "asp.schema-reference-decision-registry.v1",
                "schemaVersion": "1",
                "entries": [_entry()],
            }
        ),
        encoding="utf-8",
    )
    contract_path = tmp_path / "contract.json"
    contract_path.write_text(
        json.dumps({"type": "object", "required": ["missing"]}),
        encoding="utf-8",
    )
    _, diagnostics = load_reference_decisions(registry_path, contract_path)
    assert any(
        item["code"] == "invalid-reference-decision-registry-contract"
        for item in diagnostics
    )
