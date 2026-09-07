import copy
import json
from pathlib import Path

import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas/runtime-artifact-execution-closure-member.v1.schema.json"


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def member(kind: str) -> dict:
    entries = {
        "provider-registration": [{
            "providerId": "asp-rust", "languageId": "rust",
            "registrationDigest": digest("1"), "artifactMember": "asp-rust",
        }],
        "provider-artifact-set": [{
            "providerId": "asp-rust", "artifactMember": "asp-rust",
            "artifactDigest": digest("2"),
        }],
        "evaluator-policy": [{"id": "query-admission", "digest": digest("4")}],
        "evaluator-abi": [{"id": "query-playbook-v1", "digest": digest("5")}],
        "schema-bundle": [{"languageId": "rust", "schemaDigest": digest("6")}],
    }[kind]
    return {
        "schemaId": "agent.semantic-protocols.runtime-artifact-execution-closure-member",
        "schemaVersion": "1",
        "memberKind": kind,
        "entries": entries,
    }


def validator() -> jsonschema.Draft202012Validator:
    return schema_validator_for(SCHEMA)


@pytest.mark.parametrize("kind", [
    "provider-registration", "provider-artifact-set", "evaluator-policy",
    "evaluator-abi", "schema-bundle",
])
def test_each_execution_closure_member_has_one_typed_shape(kind: str) -> None:
    validator().validate(member(kind))


def test_member_kind_cannot_relabel_another_member_payload() -> None:
    packet = member("provider-registration")
    packet["memberKind"] = "evaluator-abi"
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_activation_generation_is_not_a_closure_member() -> None:
    packet = member("schema-bundle")
    packet["activationGeneration"] = 86
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_empty_or_digest_only_member_is_rejected() -> None:
    packet = member("evaluator-policy")
    packet["entries"] = []
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)

    digest_only = copy.deepcopy(packet)
    digest_only.pop("entries")
    digest_only["digest"] = digest("7")
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(digest_only)


@pytest.mark.parametrize("kind", ["provider-registration", "provider-artifact-set"])
def test_an_explicit_empty_provider_set_is_a_valid_bootstrap_member(kind: str) -> None:
    packet = member(kind)
    packet["entries"] = []
    validator().validate(packet)
