# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the Runtime EvidenceGraph derivation receipt contract."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from unit.schema_validation import schema_validator_for


def _schema_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "schemas"
        / "semantic-evidence-graph-derivation-receipt.v1.schema.json"
    )


def _receipt() -> dict[str, object]:
    digest = f"blake3-256:{'a' * 64}"
    return {
        "schemaId": "agent.semantic-protocols.evidence-graph-derivation-receipt",
        "schemaVersion": "1",
        "programId": "mrr.evidence-graph.runtime",
        "programDigest": digest,
        "sourceGenerationDigest": digest,
        "sourceFactDigest": digest,
        "graphDigest": digest,
        "inputNodeCount": 2,
        "inputFactCount": 1,
        "derivedEdgeCount": 1,
        "rules": [
            {
                "ruleId": (
                    "mrr.evidence-graph.runtime.invariant-verified-by-receipt"
                ),
                "gqlPlanDigest": digest,
                "inputRelation": "VERIFIED_BY",
                "outputEdgeKind": "verified-by",
            }
        ],
        "complete": True,
    }


def test_evidence_graph_derivation_receipt_schema_is_valid() -> None:
    Draft202012Validator.check_schema(json.loads(_schema_path().read_text()))


def test_evidence_graph_derivation_receipt_accepts_complete_runtime_receipt() -> None:
    schema_validator_for(_schema_path()).validate(_receipt())


def test_evidence_graph_derivation_receipt_rejects_version_suffix_in_program_id() -> None:
    receipt = _receipt()
    receipt["programId"] = "mrr.evidence-graph.runtime.v1"
    errors = list(schema_validator_for(_schema_path()).iter_errors(receipt))
    assert errors
