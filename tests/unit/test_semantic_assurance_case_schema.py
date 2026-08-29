"""Validate the semantic assurance case schema contract."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from unit.schema_validation import schema_validator_for


def _schema_path() -> Path:
    return Path(__file__).resolve().parents[2] / "schemas" / "semantic-assurance-case.v1.schema.json"


def _load_schema() -> dict:
    return json.loads(_schema_path().read_text())


def test_semantic_assurance_case_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_schema())


def test_semantic_assurance_case_accepts_graph_derived_cases() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.semantic-assurance-case",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.assurance-case",
        "protocolVersion": "1",
        "caseSetId": "rust.assurance.case",
        "producer": {
            "languageId": "rust",
            "providerId": "asp-rust",
            "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
        },
        "project": {"root": "."},
        "summary": {
            "cases": 1,
            "claims": 1,
            "supportedClaims": 0,
            "openGaps": 1,
            "staleItems": 1,
        },
        "cases": [
            {
                "caseId": "case:invariant.agent-r027:src.model.rs:42",
                "claim": {
                    "claimId": "claim:agent-r027:src.model.rs:42",
                    "kind": "invariant",
                    "statement": "semantic fields need named type",
                    "targetNodeId": "invariant:agent-r027:src.model.rs:42",
                    "severity": "warning",
                },
                "status": "needs-review",
                "subjectNodeId": "invariant:agent-r027:src.model.rs:42",
                "ownerPath": "src/model.rs",
                "supportedBy": [
                    {
                        "nodeId": "verification-receipt:rust.expect-test.src-model",
                        "kind": "verification-receipt",
                        "label": "rust.expect-test.src-model",
                        "status": "current",
                    }
                ],
                "reviewedBy": [
                    {
                        "nodeId": "review-packet:rust.review.packet",
                        "kind": "review-packet",
                        "label": "rust.review.packet",
                    }
                ],
                "waivedBy": [
                    {
                        "nodeId": "waiver:waiver.agent-r027.src-model",
                        "kind": "waiver",
                        "label": "waiver.agent-r027.src-model",
                        "status": "stale",
                    }
                ],
                "actions": [
                    {
                        "nodeId": "review-action:run-receipt.agent-r027.src-model.expect-test",
                        "actionId": "run-receipt.agent-r027.src-model.expect-test",
                        "summary": "Run expect-test",
                        "priority": "p0",
                    }
                ],
                "gaps": [
                    {
                        "gapId": "gap:agent-r027:src.model.rs:42:expect-test",
                        "sourceGapId": "gap:agent-r027:src.model.rs:42:expect-test",
                        "ownerPath": "src/model.rs",
                        "summary": "no passed expect-test receipt linked to candidate",
                        "severity": "warning",
                    }
                ],
            }
        ],
    }

    schema_validator_for(_schema_path()).validate(value)


def test_semantic_assurance_case_rejects_absolute_owner_paths() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.semantic-assurance-case",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.assurance-case",
        "protocolVersion": "1",
        "caseSetId": "rust.assurance.case",
        "producer": {
            "languageId": "rust",
            "providerId": "asp-rust",
            "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
        },
        "project": {"root": "."},
        "summary": {
            "cases": 1,
            "claims": 1,
            "supportedClaims": 0,
            "openGaps": 0,
            "staleItems": 0,
        },
        "cases": [
            {
                "caseId": "case:bad",
                "claim": {
                    "claimId": "claim:bad",
                    "kind": "custom",
                    "statement": "bad path",
                },
                "status": "unknown",
                "ownerPath": "/tmp/src/model.rs",
            }
        ],
    }

    errors = list(schema_validator_for(_schema_path()).iter_errors(value))
    assert errors
