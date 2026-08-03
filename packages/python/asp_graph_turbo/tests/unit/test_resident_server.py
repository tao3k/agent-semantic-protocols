from __future__ import annotations

import copy
import io
import json
import subprocess
import sys
from pathlib import Path

import pytest

from asp_graph_turbo.resident_server import (
    ResidentGraphTurboSession,
    ResidentProtocolError,
    serve_json_lines,
)


ROOT = Path(__file__).resolve().parents[5]


def graph_request(snapshot: str, generation: str) -> dict[str, object]:
    request = json.loads(
        (ROOT / "sandtables/fixtures/asp/graph-turbo-owner-query.json").read_text(
            encoding="utf-8"
        )
    )
    request = copy.deepcopy(request)
    request["sourceSnapshot"]["rootDigest"] = snapshot
    request["workspaceGeneration"]["rootDigest"] = generation
    return request


def message(kind: str, request_id: str, **fields: object) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-message",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "messageKind": kind,
        "requestId": request_id,
        **fields,
    }


def process_handshake(**fields: object) -> dict[str, object]:
    return message(
        "process-handshake",
        "handshake",
        runtimeArtifactDigest=f"blake3-256:{'c' * 64}",
        executionCommandDigest=f"blake3-256:{'d' * 64}",
        **fields,
    )


def test_one_process_multiplexes_two_exact_graph_sessions() -> None:
    snapshot_a = "a" * 64
    snapshot_b = "b" * 64
    identity_a = {
        "sessionId": "session-a",
        "nodeId": "router-a",
        "snapshotDigest": snapshot_a,
        "workspaceGenerationRootDigest": "generation-a",
        "routeId": "route-a",
    }
    identity_b = {
        "sessionId": "session-b",
        "nodeId": "router-b",
        "snapshotDigest": snapshot_b,
        "workspaceGenerationRootDigest": "generation-b",
        "routeId": "route-b",
    }
    messages = [
        process_handshake(),
        message(
            "rank",
            "rank-a-cold",
            **identity_a,
            request=graph_request(snapshot_a, "generation-a"),
        ),
        message(
            "rank",
            "rank-b-cold",
            **identity_b,
            request=graph_request(snapshot_b, "generation-b"),
        ),
        message(
            "rank",
            "rank-a-warm",
            **identity_a,
            request=graph_request(snapshot_a, "generation-a"),
        ),
        message("shutdown", "shutdown"),
    ]
    reader = io.StringIO("".join(json.dumps(item) + "\n" for item in messages))
    writer = io.StringIO()

    assert serve_json_lines(reader, writer) == 0
    receipts = [json.loads(line) for line in writer.getvalue().splitlines()]

    assert receipts[0]["status"] == "process-handshake-accepted"
    assert receipts[1]["graphSessionIdentity"] == identity_a
    assert receipts[1]["accounting"]["graphTurboInvocations"] == 1
    assert receipts[2]["graphSessionIdentity"] == identity_b
    assert receipts[2]["accounting"]["graphTurboInvocations"] == 1
    assert receipts[3]["graphSessionIdentity"] == identity_a
    assert receipts[3]["accounting"]["graphTurboInvocations"] == 2
    assert receipts[4]["status"] == "shutdown-accepted"


def test_outer_identity_cannot_substitute_inner_snapshot() -> None:
    snapshot = "a" * 64
    messages = [
        process_handshake(),
        message(
            "rank",
            "rank-forged",
            sessionId="session-a",
            nodeId="router-a",
            snapshotDigest=snapshot,
            workspaceGenerationRootDigest="generation-a",
            routeId="route-a",
            request=graph_request("b" * 64, "generation-a"),
        ),
        message("shutdown", "shutdown"),
    ]
    writer = io.StringIO()

    assert serve_json_lines(
        io.StringIO("".join(json.dumps(item) + "\n" for item in messages)), writer
    ) == 0
    receipts = [json.loads(line) for line in writer.getvalue().splitlines()]

    assert receipts[1]["status"] == "rejected"
    assert receipts[1]["failure"]["code"] == "snapshot-identity-mismatch"
    assert receipts[2]["status"] == "shutdown-accepted"


def test_process_handshake_cannot_claim_semantic_identity() -> None:
    session = ResidentGraphTurboSession()
    with pytest.raises(
        ResidentProtocolError, match="cannot carry graph-session identity"
    ) as error:
        session.handle(process_handshake(sessionId="forged-session"))
    assert error.value.code == "process-semantic-identity-forbidden"


def test_restarted_process_requires_a_new_process_handshake() -> None:
    session = ResidentGraphTurboSession()
    with pytest.raises(ResidentProtocolError, match="process-handshake is required"):
        session.handle(
            message(
                "rank",
                "rank-before-handshake",
                sessionId="session-a",
                nodeId="router-a",
                snapshotDigest="a" * 64,
                workspaceGenerationRootDigest="generation-a",
                routeId="route-a",
                request=graph_request("a" * 64, "generation-a"),
            )
        )


def test_unknown_schema_version_is_rejected() -> None:
    handshake = process_handshake()
    handshake["schemaVersion"] = "2"
    with pytest.raises(ResidentProtocolError) as error:
        ResidentGraphTurboSession().handle(handshake)
    assert error.value.code == "unsupported-schema-version"


def test_json_line_process_uses_the_single_v1_contract() -> None:
    completed = subprocess.run(
        [sys.executable, "-m", "asp_graph_turbo.resident_server"],
        input=json.dumps(process_handshake())
        + "\n"
        + json.dumps(message("shutdown", "shutdown"))
        + "\n",
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    assert completed.returncode == 0, completed.stderr
    receipts = [json.loads(line) for line in completed.stdout.splitlines()]
    assert [receipt["schemaVersion"] for receipt in receipts] == ["1", "1"]
    assert [receipt["status"] for receipt in receipts] == [
        "process-handshake-accepted",
        "shutdown-accepted",
    ]
