"""Exercise ASP Python Graphs session lifecycle and immutable generation identity."""

from __future__ import annotations

import copy

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, message, rank_payload


def _hello_session() -> AspPythonGraphsSession:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    return session


def test_old_resident_schema_is_rejected() -> None:
    packet = message("health", "health-1")
    packet["schemaId"] = "agent.semantic-protocols.invalid-legacy-service"
    with pytest.raises(ServiceProtocolError) as error:
        AspPythonGraphsSession().handle(packet)
    assert error.value.code == "unsupported-schema"


def test_timeline_payload_rejects_private_fields() -> None:
    session = _hello_session()
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
    session = _hello_session()
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message("health", "health-1"))
    assert error.value.code == "non-monotonic-sequence"


def test_same_generation_reuses_immutable_graph_and_rejects_conflicting_snapshot() -> (
    None
):
    session = _hello_session()
    graph = rank_payload()["graph"]
    first = session.handle(
        message(
            "open-generation",
            "open-1",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": graph},
        )
    )
    second = session.handle(
        message(
            "open-generation",
            "open-2",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": copy.deepcopy(graph)},
        )
    )
    assert first["payload"]["generationLoads"] == 1  # type: ignore[index]
    assert second["payload"]["generationLoads"] == 0  # type: ignore[index]

    changed = copy.deepcopy(graph)
    changed["nodes"] = list(changed["nodes"]) + [  # type: ignore[index]
        {"id": "new-node", "kind": "test"}
    ]
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "open-generation",
                "open-3",
                workspaceIdentity="workspace-a",
                generationDigest=DIGEST_A,
                payload={"graph": changed},
            )
        )
    assert error.value.code == "generation-graph-mismatch"


def test_generation_token_is_part_of_resident_identity() -> None:
    session = _hello_session()
    graph = rank_payload()["graph"]
    first = session.handle(
        message(
            "open-generation",
            "open-1",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": graph},
        )
    )
    second = session.handle(
        message(
            "open-generation",
            "open-2",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            generationToken=2,
            payload={"graph": graph},
        )
    )
    assert first["payload"]["loadedGenerationCount"] == 1  # type: ignore[index]
    assert second["payload"]["loadedGenerationCount"] == 2  # type: ignore[index]


def test_shutdown_is_terminal_and_drains_all_session_owned_state() -> None:
    session = _hello_session()
    session.handle(
        message(
            "open-generation",
            "open-1",
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": rank_payload()["graph"]},
        )
    )
    session.register_request("eval-pending")
    session.handle(
        message("cancel", "cancel-shutdown", sequence=3, cancellationId="eval-pending")
    )

    terminal = session.handle(message("shutdown", "shutdown-1", sequence=4))

    assert terminal["payload"]["state"] == "cancelled"  # type: ignore[index]
    assert session.closed
    assert session.loaded_generations == {}
    assert session.generation_packets == {}
    assert session._active_requests == set()
    assert session._admitted_requests == set()
    assert session._cancelled_requests == set()
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "hello",
                "hello-after-shutdown",
                sequence=5,
                runtimeArtifactDigest=DIGEST_A,
                executionArtifactDigest=DIGEST_B,
            )
        )
    assert error.value.code == "process-closed"
