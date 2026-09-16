# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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


def test_artifact_bound_asp_python_graphs_receipt_is_valid() -> None:
    receipt = progressive_receipt()
    receipt.update(
        requestedEngine="asp-python-graphs",
        selectedEngine="asp-python-graphs",
    )
    VALIDATOR.validate(receipt)


def test_missing_candidate_identity_can_only_be_blocked_without_selection() -> None:
    receipt = progressive_receipt()
    receipt.update(
        selectedEngine=None,
        decision="blocked",
        reason="candidate-artifact-identity-missing",
        candidateArtifactIdentity=None,
    )

    VALIDATOR.validate(receipt)


@pytest.mark.parametrize("removed_engine", ["graph-turbo", "legacy-graph-turbo"])
def test_removed_engine_identifier_is_rejected(removed_engine: str) -> None:
    receipt = progressive_receipt()
    receipt.update(
        requestedEngine=removed_engine,
        selectedEngine=removed_engine,
        decision="ready",
    )

    with pytest.raises(ValidationError):
        VALIDATOR.validate(receipt)


def test_malformed_candidate_digest_is_rejected() -> None:
    receipt = progressive_receipt()
    receipt["candidateArtifactIdentity"]["runtimeDigest"] = "sha256:not-a-digest"  # type: ignore[index]

    with pytest.raises(ValidationError):
        VALIDATOR.validate(receipt)
