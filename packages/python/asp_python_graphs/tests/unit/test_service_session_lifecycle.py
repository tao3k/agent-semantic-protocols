"""Exercise the stateless Python graph-construction lifecycle."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import (
    DIGEST_A,
    DIGEST_B,
    generation_payload,
    message,
    resident_evaluation_payload,
)


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


@pytest.mark.parametrize(
    "legacy_kind", ["open-generation", "evaluate", "search-evidence"]
)
def test_request_time_generation_authority_is_removed(legacy_kind: str) -> None:
    session = _hello_session()
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message(legacy_kind, f"legacy-{legacy_kind}", sequence=2))
    assert error.value.code == "unsupported-message-kind"


def _retain_generation(
    session: AspPythonGraphsSession, sequence: int = 2
) -> dict[str, object]:
    return session.handle(
        message(
            "generation-graph",
            f"generation-{sequence}",
            sequence=sequence,
            workspaceIdentity="workspace-test",
            generationDigest=DIGEST_B,
            payload=generation_payload(),
        )
    )


def test_generation_graph_is_retained_and_idempotent_for_exact_identity() -> None:
    session = _hello_session()
    first = _retain_generation(session)
    second = _retain_generation(session, sequence=3)
    assert first["payload"]["retained"] is True  # type: ignore[index]
    assert second["payload"]["result"] == first["payload"]["result"]  # type: ignore[index]
    health = session.handle(message("health", "health-1", sequence=4))
    assert health["payload"]["retainedGenerationCount"] == 1  # type: ignore[index]


def test_warm_evaluation_uses_only_controls_and_exact_generation_identity() -> None:
    session = _hello_session()
    retained = _retain_generation(session)
    artifact_digest = retained["payload"]["result"]["artifactDigest"]  # type: ignore[index]
    payload = resident_evaluation_payload()
    assert "graph" not in payload
    receipt = session.handle(
        message(
            "evaluate-resident",
            "evaluate-resident-1",
            sequence=3,
            workspaceIdentity="workspace-test",
            generationDigest=DIGEST_B,
            payload=payload,
        )
    )
    assert receipt["workspaceIdentity"] == "workspace-test"
    assert receipt["generationDigest"] == DIGEST_B
    assert receipt["payload"]["graphArtifactDigest"] == artifact_digest  # type: ignore[index]
    assert receipt["payload"]["result"]["rank"]  # type: ignore[index]
    assert receipt["payload"]["result"]["rank"] == ["owner:src/a.py"]  # type: ignore[index]


def test_candidate_projection_rejects_nodes_outside_entry_frontier() -> None:
    session = _hello_session()
    _retain_generation(session)
    payload = resident_evaluation_payload()
    payload["candidateNodeIds"] = ["owner:src/b.py"]
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "evaluate-resident",
                "evaluate-invalid-candidate-frontier",
                sequence=3,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_B,
                payload=payload,
            )
        )
    assert error.value.code == "invalid-resident-evaluation"


def test_wrong_generation_fails_closed_without_retransmitting_graph() -> None:
    session = _hello_session()
    _retain_generation(session)
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "evaluate-resident",
                "evaluate-wrong-generation",
                sequence=3,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_A,
                payload=resident_evaluation_payload(),
            )
        )
    assert error.value.code == "resident-generation-unavailable"


@pytest.mark.parametrize(
    "surface", ["search-pipe", "search-rg", "search-lexical", "search-owner"]
)
def test_retired_search_surfaces_are_rejected(surface: str) -> None:
    session = _hello_session()
    payload = resident_evaluation_payload()
    payload["surface"] = surface
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "evaluate-resident",
                "evaluate-retired-surface",
                sequence=2,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_B,
                payload=payload,
            )
        )
    assert error.value.code == "invalid-resident-evaluation"


def test_release_removes_only_the_exact_generation() -> None:
    session = _hello_session()
    _retain_generation(session)
    released = session.handle(
        message(
            "release-generation",
            "release-generation-1",
            sequence=3,
            workspaceIdentity="workspace-test",
            generationDigest=DIGEST_B,
        )
    )
    assert released["payload"]["state"] == "released"  # type: ignore[index]
    health = session.handle(message("health", "health-after-release", sequence=4))
    assert health["payload"]["retainedGenerationCount"] == 0  # type: ignore[index]


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


def test_shutdown_is_terminal_and_clears_request_state() -> None:
    session = _hello_session()
    session.register_request("compile-pending")
    session.handle(
        message(
            "cancel", "cancel-shutdown", sequence=2, cancellationId="compile-pending"
        )
    )
    terminal = session.handle(message("shutdown", "shutdown-1", sequence=3))
    assert terminal["payload"]["state"] == "cancelled"  # type: ignore[index]
    assert session.closed
    assert session._active_requests == set()
    assert session._admitted_requests == set()
    assert session._cancelled_requests == set()
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "hello",
                "hello-after-shutdown",
                sequence=4,
                runtimeArtifactDigest=DIGEST_A,
                executionArtifactDigest=DIGEST_B,
            )
        )
    assert error.value.code == "process-closed"
