from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas" / "semantic-graph-turbo-resident-message.v1.schema.json"
RECEIPT_SCHEMA = ROOT / "schemas" / "semantic-graph-turbo-resident-receipt.v1.schema.json"
CACHE_SCHEMA = ROOT / "schemas" / "semantic-graph-turbo-cache-entry.v1.schema.json"
RUNTIME_RECEIPT_SCHEMA = (
    ROOT / "schemas" / "semantic-graph-turbo-runtime-receipt.v1.schema.json"
)
REQUEST_SCHEMA = ROOT / "schemas" / "semantic-graph-turbo-request.v1.schema.json"
SOURCE_SNAPSHOT_SCHEMA = ROOT / "schemas" / "source-snapshot-evidence.v1.schema.json"
GRAPH_TURBO_REQUEST = ROOT / "sandtables" / "fixtures" / "asp" / "graph-turbo-owner-query.json"
FIXTURES = ROOT / "schemas" / "fixtures" / "semantic-graph-turbo-resident"


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def message_validator() -> Draft202012Validator:
    request_schema = load_json(REQUEST_SCHEMA)
    source_snapshot_schema = load_json(SOURCE_SNAPSHOT_SCHEMA)
    registry = Registry().with_resource(
        str(request_schema["$id"]), Resource.from_contents(request_schema)
    ).with_resource(
        str(source_snapshot_schema["$id"]),
        Resource.from_contents(source_snapshot_schema),
    )
    return Draft202012Validator(load_json(SCHEMA), registry=registry)


def errors_for(name: str) -> list[str]:
    return [
        error.message
        for error in message_validator().iter_errors(load_json(FIXTURES / name))
    ]


def receipt_errors_for(name: str) -> list[str]:
    validator = Draft202012Validator(load_json(RECEIPT_SCHEMA))
    return [error.message for error in validator.iter_errors(load_json(FIXTURES / name))]


def cache_errors_for(name: str) -> list[str]:
    validator = Draft202012Validator(load_json(CACHE_SCHEMA))
    return [error.message for error in validator.iter_errors(load_json(FIXTURES / name))]


def test_process_handshake_fixture_is_valid() -> None:
    assert errors_for("valid-handshake.v1.json") == []


def test_process_handshake_does_not_own_semantic_identity() -> None:
    with pytest.raises(ValidationError):
        message_validator().validate(
            load_json(FIXTURES / "invalid-handshake-semantic-identity.v1.json")
        )


def test_resident_rank_envelope_reuses_the_existing_graph_turbo_request_contract() -> None:
    request = copy.deepcopy(load_json(GRAPH_TURBO_REQUEST))
    request["sourceSnapshot"]["rootDigest"] = "a" * 64
    request["workspaceGeneration"]["rootDigest"] = "generation:one"
    message = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-message",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "messageKind": "rank",
        "requestId": "request:rank",
        "sessionId": "session:one",
        "nodeId": "router:one",
        "snapshotDigest": "a" * 64,
        "workspaceGenerationRootDigest": "generation:one",
        "routeId": "route:one",
        "request": request,
    }

    assert list(message_validator().iter_errors(message)) == []


def test_process_and_two_distinct_graph_session_receipts_are_valid() -> None:
    validator = Draft202012Validator(load_json(RECEIPT_SCHEMA))
    handshake = load_json(FIXTURES / "valid-process-handshake-receipt.v1.json")
    session_a = load_json(FIXTURES / "valid-rank-receipt.v1.json")
    session_b = load_json(FIXTURES / "valid-session-b-rank-receipt.v1.json")

    for receipt in (handshake, session_a, session_b):
        validator.validate(receipt)

    identity_a = session_a["graphSessionIdentity"]
    identity_b = session_b["graphSessionIdentity"]
    assert identity_a != identity_b
    assert identity_a["sessionId"] != identity_b["sessionId"]
    assert identity_a["snapshotDigest"] != identity_b["snapshotDigest"]


def test_resident_rank_receipt_rejects_python_proof_authority() -> None:
    assert any(
        "candidate" in message
        for message in receipt_errors_for("invalid-python-proved-authority.v1.json")
    )


def test_graph_turbo_cache_entry_binds_snapshot_and_candidate_authority() -> None:
    assert cache_errors_for("valid-cache-entry.v1.json") == []


def test_graph_turbo_cache_entry_rejects_proved_authority() -> None:
    assert any(
        "candidate" in message
        for message in cache_errors_for("invalid-cache-entry-proved.v1.json")
    )


def test_runtime_receipt_binds_process_timing_hops_and_turso_status() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-runtime-receipt",
        "schemaVersion": "1",
        "sessionId": "session:one",
        "snapshotDigest": "a" * 64,
        "processId": 1,
        "runtimeArtifact": "/runtime/python3",
        "executionCommandDigest": "blake3-256:graph-turbo-resident-v1",
        "coldRankMicros": 10,
        "warmRankMicros": 5,
        "graphTurboInvocations": 2,
        "semanticGraphHops": 0,
        "executedGraphHops": 0,
        "authority": "candidate",
        "tursoExactStatus": "hit",
        "tursoStaleSnapshotStatus": "miss",
        "tursoRestartStatus": "hit",
    }
    validator = Draft202012Validator(load_json(RUNTIME_RECEIPT_SCHEMA))

    assert list(validator.iter_errors(receipt)) == []
