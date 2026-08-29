"""Exercise ASP Python Graphs generation ownership and evaluation identity."""

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession


ROOT = Path(__file__).resolve().parents[5]
DIGEST_A = f"blake3-256:{'a' * 64}"
DIGEST_B = f"blake3-256:{'b' * 64}"


def rank_payload() -> dict[str, object]:
    packet = json.loads(
        (ROOT / "sandtables/fixtures/asp/graph-turbo-owner-query.json").read_text()
    )
    return {
        "graph": copy.deepcopy(packet["graph"]),
        "seedIds": copy.deepcopy(packet["seedIds"]),
        "kindBudgets": copy.deepcopy(packet["kindBudgets"]),
        "windowMerge": copy.deepcopy(packet["windowMerge"]),
        "pathBudget": packet["pathBudget"],
        "pathMaxHops": packet["pathMaxHops"],
        "cache": copy.deepcopy(packet["cache"]),
        "queryClauses": copy.deepcopy(packet.get("queryClauses", [])),
    }


def message(kind: str, request_id: str, **fields: object) -> dict[str, object]:
    sequence_by_request = {
        "hello-1": 1,
        "open-1": 2,
        "open-2": 3,
        "open-3": 4,
        "evaluate-1": 3,
        "evaluate-stale": 2,
        "cancel-1": 3,
        "evaluate-cancelled": 4,
        "eval-1": 4,
    }
    return {
        "schemaId": "agent.semantic-protocols.asp-python-graphs-session",
        "schemaVersion": "1",
        "sessionId": "session-a",
        "serviceEpoch": "epoch-a",
        "requestId": request_id,
        "sequence": sequence_by_request.get(request_id, 1),
        "generationToken": 1,
        "messageKind": kind,
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-test",
        "payload": {},
        **fields,
    }


def test_one_service_session_reuses_one_loaded_generation() -> None:
    session = AspPythonGraphsSession()
    hello = session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    loaded = session.handle(
        message(
            "open-generation",
            "open-1",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": rank_payload()["graph"]},
        )
    )
    request_payload = rank_payload()
    request_payload.pop("graph")
    evaluated = session.handle(
        message(
            "evaluate",
            "evaluate-1",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={
                "terms": ["runtime"],
                "profile": "owner-query",
                "budget": 8,
                "rankPayload": request_payload,
            },
        )
    )

    assert hello["payload"]["loadedGenerationCount"] == 0  # type: ignore[index]
    assert loaded["payload"]["generationLoads"] == 1  # type: ignore[index]
    assert evaluated["payload"]["generationLoads"] == 0  # type: ignore[index]
    assert evaluated["workspaceIdentity"] == "workspace-a"
    assert evaluated["generationDigest"] == DIGEST_A


def test_cross_generation_evaluation_is_rejected() -> None:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    with pytest.raises(ServiceProtocolError, match="open-generation") as error:
        session.handle(
            message(
                "evaluate",
                "evaluate-stale",
                workspaceIdentity="workspace-a",
                generationDigest=DIGEST_B,
                payload={"terms": ["runtime"], "rankPayload": {}},
            )
        )
    assert error.value.code == "generation-not-loaded"


def test_old_resident_schema_is_rejected() -> None:
    packet = message("health", "health-1")
    packet["schemaId"] = "agent.semantic-protocols.invalid-legacy-service"
    with pytest.raises(ServiceProtocolError) as error:
        AspPythonGraphsSession().handle(packet)
    assert error.value.code == "unsupported-schema"


def test_timeline_payload_rejects_private_fields() -> None:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "timeline",
                "timeline-1",
                sequence=2,
                payload={"eventPacket": {}, "graphTurboResident": True},
            )
        )
    assert error.value.code == "unknown-timeline-field"


def test_duplicate_session_sequence_is_rejected() -> None:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message("health", "health-1"))
    assert error.value.code == "non-monotonic-sequence"


def test_same_generation_reuses_immutable_graph_and_rejects_conflicting_snapshot() -> None:
    session = AspPythonGraphsSession()
    session.handle(message("hello", "hello-1", runtimeArtifactDigest=DIGEST_A,
                           executionArtifactDigest=DIGEST_B))
    graph = rank_payload()["graph"]
    first = session.handle(message("open-generation", "open-1", workspaceIdentity="workspace-a",
                                   generationDigest=DIGEST_A, payload={"graph": graph}))
    second = session.handle(message("open-generation", "open-2", workspaceIdentity="workspace-a",
                                    generationDigest=DIGEST_A, payload={"graph": copy.deepcopy(graph)}))
    assert first["payload"]["generationLoads"] == 1  # type: ignore[index]
    assert second["payload"]["generationLoads"] == 0  # type: ignore[index]
    changed = copy.deepcopy(graph)
    changed["nodes"] = list(changed["nodes"]) + [{"id": "new-node", "kind": "test"}]  # type: ignore[index]
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message("open-generation", "open-3", workspaceIdentity="workspace-a",
                               generationDigest=DIGEST_A, payload={"graph": changed}))
    assert error.value.code == "generation-graph-mismatch"


def test_generation_token_is_part_of_resident_identity() -> None:
    session = AspPythonGraphsSession()
    session.handle(message("hello", "hello-1", runtimeArtifactDigest=DIGEST_A,
                           executionArtifactDigest=DIGEST_B))
    graph = rank_payload()["graph"]
    first = session.handle(message("open-generation", "open-1", workspaceIdentity="workspace-a",
                                   generationDigest=DIGEST_A, payload={"graph": graph}))
    second = session.handle(message("open-generation", "open-2", workspaceIdentity="workspace-a",
                                    generationDigest=DIGEST_A, generationToken=2,
                                    payload={"graph": graph}))
    assert first["payload"]["loadedGenerationCount"] == 1  # type: ignore[index]
    assert second["payload"]["loadedGenerationCount"] == 2  # type: ignore[index]


def test_shutdown_is_terminal_and_drains_all_session_owned_state() -> None:
    session = AspPythonGraphsSession()
    session.handle(message("hello", "hello-1", runtimeArtifactDigest=DIGEST_A,
                           executionArtifactDigest=DIGEST_B))
    session.handle(message("open-generation", "open-1", workspaceIdentity="workspace-a",
                           generationDigest=DIGEST_A,
                           payload={"graph": rank_payload()["graph"]}))
    session.register_request("eval-pending")
    session.handle(message("cancel", "cancel-shutdown", sequence=3,
                           cancellationId="eval-pending"))

    terminal = session.handle(message("shutdown", "shutdown-1", sequence=4))

    assert terminal["payload"]["state"] == "cancelled"  # type: ignore[index]
    assert session.closed
    assert session.loaded_generations == {}
    assert session.generation_packets == {}
    assert session._active_requests == set()
    assert session._admitted_requests == set()
    assert session._cancelled_requests == set()
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message("hello", "hello-after-shutdown", sequence=5,
                               runtimeArtifactDigest=DIGEST_A,
                               executionArtifactDigest=DIGEST_B))
    assert error.value.code == "process-closed"
