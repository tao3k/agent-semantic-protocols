# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the semantic review packet schema contract."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from unit.schema_validation import schema_validator_for


def _schema_path() -> Path:
    return Path(__file__).resolve().parents[2] / "schemas" / "semantic-review-packet.v1.schema.json"


def _load_schema() -> dict:
    return json.loads(_schema_path().read_text())


def test_semantic_review_packet_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_schema())


def test_semantic_review_packet_accepts_reviewer_first_artifact() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.semantic-review-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.review-packet",
        "protocolVersion": "1",
        "packetId": "rust.review.packet",
        "producer": {
            "languageId": "rust",
            "providerId": "asp-rust",
            "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
        },
        "project": {"root": "."},
        "summary": {
            "changedInvariants": 1,
            "changedBehavior": 1,
            "missingReceipts": 1,
            "staleWaivers": 1,
            "determinismObservations": 2,
            "proofClaims": 1,
        },
        "changedInvariants": [
            {
                "invariantId": "agent-r027:src.model.rs:42",
                "sourceRuleId": "AGENT-R027",
                "kind": "public-data-primitive-fields",
                "severity": "warning",
                "title": "semantic fields need named type",
                "hypothesis": "public data shape should not expose stringly fields",
                "location": {"path": "src/model.rs", "line": 42, "column": 0},
                "requiredReceipts": ["cargo-check", "expect-test"],
            }
        ],
        "changedBehavior": [
            {
                "snapshotId": "rust.behavior.src-model",
                "status": "changed",
                "subject": "src/model.rs",
                "summary": "expect-test output changed",
                "receiptIds": ["rust.expect-test.src-model"],
                "candidateIds": ["agent-r027:src.model.rs:42"],
            }
        ],
        "missingReceipts": [
            {
                "invariantId": "agent-r027:src.model.rs:42",
                "receiptKind": "expect-test",
                "reason": "no passed expect-test receipt linked to candidate",
            }
        ],
        "staleWaivers": [
            {
                "waiverId": "waiver.agent-r027.src-model",
                "invariantId": "agent-r027:src.model.rs:42",
                "receiptKind": "expect-test",
                "status": "stale",
                "owner": "reviewer",
                "reason": "snapshot migration is pending",
                "expiresAt": "2026-05-01",
            }
        ],
        "determinismReadiness": [
            {
                "readinessId": "rust.determinism-readiness.project",
                "status": "needs-injection",
                "observations": 2,
                "suggestions": 2,
            }
        ],
        "proofPilots": [
            {
                "proofId": "rust.proof.dependency-graph-acyclicity",
                "target": "owner dependency graph cycle detection",
                "status": "proved-bounded",
                "claims": 1,
                "checks": 1,
            }
        ],
        "reviewActions": [
            {
                "actionId": "run-receipt.agent-r027.src-model.expect-test",
                "kind": "run-receipt",
                "priority": "p0",
                "summary": "Run expect-test for agent-r027:src.model.rs:42",
                "targetId": "agent-r027:src.model.rs:42",
            }
        ],
    }

    schema_validator_for(_schema_path()).validate(value)


def test_semantic_review_packet_rejects_absolute_invariant_paths() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.semantic-review-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.review-packet",
        "protocolVersion": "1",
        "packetId": "rust.review.packet",
        "producer": {
            "languageId": "rust",
            "providerId": "asp-rust",
            "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
        },
        "project": {"root": "."},
        "summary": {
            "changedInvariants": 1,
            "changedBehavior": 0,
            "missingReceipts": 0,
            "staleWaivers": 0,
            "determinismObservations": 0,
            "proofClaims": 0,
        },
        "changedInvariants": [
            {
                "invariantId": "agent-r027:src.model.rs:42",
                "sourceRuleId": "AGENT-R027",
                "kind": "public-data-primitive-fields",
                "severity": "warning",
                "title": "semantic fields need named type",
                "hypothesis": "public data shape should not expose stringly fields",
                "location": {"path": "/tmp/src/model.rs", "line": 42, "column": 0},
                "requiredReceipts": ["cargo-check"],
            }
        ],
        "changedBehavior": [],
        "missingReceipts": [],
        "staleWaivers": [],
        "reviewActions": [],
    }

    errors = list(schema_validator_for(_schema_path()).iter_errors(value))
    assert errors
