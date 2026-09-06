# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


SCHEMA_PATH = (
    Path(__file__).parents[3]
    / "schemas"
    / "fallback-reflection-receipt.v1.schema.json"
)


def fallback_receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.fallback-reflection-receipt",
        "schemaVersion": "1",
        "receiptId": "receipt-1",
        "state": "reflection-admitted",
        "primary": {
            "capabilityId": "primary",
            "authority": "provider",
            "artifactDigest": "sha256:primary",
            "generation": "generation-1",
        },
        "primaryFailure": {
            "reasonKind": "typed-failure",
            "evidenceRef": "evidence-1",
            "typed": True,
        },
        "proposedFallback": {
            "capabilityId": "fallback",
            "authority": "provider",
            "artifactDigest": "sha256:fallback",
            "generation": "generation-1",
        },
        "semanticReflection": {
            "preservedSemantics": ["exact-owner"],
            "unpreservedSemantics": [],
        },
        "scope": {
            "workspaceIdentity": "workspace-1",
            "candidateGeneration": "generation-1",
            "subjects": ["owner-1"],
        },
        "cost": {
            "additionalIoOperations": 1,
            "additionalProcessCount": 0,
            "additionalDatabaseOpenCount": 0,
            "estimatedAdditionalLatencyMicros": 100,
        },
        "admission": {
            "policyRuleId": "fallback-explicit-opt-in",
            "callerOptIn": True,
            "decision": "admit",
        },
        "budget": {"limit": 1, "consumed": 1},
        "expiry": {"candidateGeneration": "generation-1"},
        "counter": {"counterId": "fallback-1", "value": 1},
    }


def validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA_PATH.read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_explicit_semantics_preserving_fallback_is_valid() -> None:
    validator().validate(fallback_receipt())


def test_admitted_fallback_requires_explicit_caller_opt_in() -> None:
    receipt = copy.deepcopy(fallback_receipt())
    receipt["admission"]["callerOptIn"] = False  # type: ignore[index]
    assert list(validator().iter_errors(receipt))


def test_semantic_loss_forces_reflection_denial() -> None:
    receipt = copy.deepcopy(fallback_receipt())
    receipt["semanticReflection"]["unpreservedSemantics"] = [  # type: ignore[index]
        "source-identity"
    ]
    assert list(validator().iter_errors(receipt))
