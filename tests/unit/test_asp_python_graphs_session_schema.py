from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/asp-python-graphs-session.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def envelope(kind: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.asp-python-graphs-session",
        "schemaVersion": "1",
        "sessionId": "graphs-session-1",
        "serviceEpoch": "graphs-epoch-1",
        "requestId": "graphs-request-1",
        "sequence": 1,
        "messageKind": kind,
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-empty",
        "payload": {},
    }


def test_hello_binds_installed_artifacts() -> None:
    packet = envelope("hello")
    packet.update(
        runtimeArtifactDigest=digest("a"),
        executionArtifactDigest=digest("b"),
    )
    VALIDATOR.validate(packet)


def test_evaluate_resident_binds_workspace_generation() -> None:
    packet = envelope("evaluate-resident")
    packet.update(
        workspaceIdentity="workspace-1",
        generationDigest=digest("c"),
    )
    VALIDATOR.validate(packet)


@pytest.mark.parametrize(
    "kind", ["generation-graph", "evaluate-resident", "release-generation"]
)
def test_generation_scoped_operation_rejects_missing_generation(kind: str) -> None:
    packet = envelope(kind)
    packet["workspaceIdentity"] = "workspace-1"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_old_graph_turbo_schema_identity_is_rejected() -> None:
    packet = envelope("health")
    packet["schemaId"] = "agent.semantic-protocols.invalid-legacy-service"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_unknown_message_kind_is_rejected() -> None:
    packet = envelope("rank")
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)
