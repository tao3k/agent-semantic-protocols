import copy
import json
from pathlib import Path

import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def commit() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.content-publication-commit",
        "schemaVersion": "1",
        "contentBinding": {
            "schemaId": "asp.content-binding",
            "schemaVersion": "1",
            "runtimeArtifactDigest": digest("1"),
            "workspaceSnapshotDigest": digest("2"),
            "sourceGenerationDigest": digest("3"),
            "sourceIndexDigest": digest("4"),
            "schemaDigest": digest("5"),
            "providerCatalogDigest": digest("6"),
            "authorityStamp": {
                "keyId": "runtime-owner",
                "canonicalDigest": digest("7"),
                "signature": "signature-1",
            },
        },
        "commitDigest": digest("8"),
        "predecessorCommitDigest": None,
        "mutationId": "mutation-1",
        "leaseId": "lease-1",
    }


def validator() -> jsonschema.Draft202012Validator:
    return schema_validator_for(SCHEMAS / "content-publication-commit.v1.schema.json")


def test_complete_publication_commit_is_schema_valid() -> None:
    validator().validate(commit())


@pytest.mark.parametrize(
    "field",
    ["contentBinding", "commitDigest", "predecessorCommitDigest", "mutationId", "leaseId"],
)
def test_publication_commit_rejects_missing_authority_or_fence_fields(field: str) -> None:
    packet = commit()
    packet.pop(field)
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_digest_only_surrogate_is_not_a_publication_commit() -> None:
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(
            {
                "schemaId": "agent.semantic-protocols.content-publication-commit",
                "schemaVersion": "1",
                "commitDigest": digest("8"),
            }
        )


def test_activation_generation_is_not_commit_identity() -> None:
    packet = copy.deepcopy(commit())
    packet["activationGeneration"] = 84
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)
