from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/semantic-search-engine-admission.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def digest(character: str) -> str:
    return f"sha256:{character * 64}"


def progressive_receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.search-engine-admission.v1",
        "schemaVersion": "1",
        "requestedEngine": "progressive-gql-logic",
        "selectedEngine": "progressive-gql-logic",
        "decision": "ready",
        "reason": None,
        "candidateArtifactIdentity": {
            "providerDigest": digest("a"),
            "runtimeDigest": digest("b"),
            "semanticContractDigest": digest("c"),
        },
    }


def test_artifact_bound_progressive_receipt_is_valid() -> None:
    VALIDATOR.validate(progressive_receipt())


def test_missing_candidate_identity_can_only_be_blocked_without_selection() -> None:
    receipt = progressive_receipt()
    receipt.update(
        selectedEngine=None,
        decision="blocked",
        reason="candidate-artifact-identity-missing",
        candidateArtifactIdentity=None,
    )

    VALIDATOR.validate(receipt)


def test_blocked_progressive_request_cannot_silently_select_legacy() -> None:
    receipt = progressive_receipt()
    receipt.update(
        selectedEngine="legacy-graph-turbo",
        decision="blocked",
        reason="candidate-artifact-identity-missing",
        candidateArtifactIdentity=None,
    )

    with pytest.raises(ValidationError):
        VALIDATOR.validate(receipt)


def test_malformed_candidate_digest_is_rejected() -> None:
    receipt = progressive_receipt()
    receipt["candidateArtifactIdentity"]["runtimeDigest"] = "sha256:not-a-digest"  # type: ignore[index]

    with pytest.raises(ValidationError):
        VALIDATOR.validate(receipt)
