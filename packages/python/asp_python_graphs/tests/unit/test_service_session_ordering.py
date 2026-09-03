"""Verify sequence and cancellation ordering for one graph service session."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession


DIGEST_A = f"blake3-256:{'a' * 64}"
DIGEST_B = f"blake3-256:{'b' * 64}"


def message(kind: str, request_id: str, sequence: int, **fields: object) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.asp-python-graphs-session",
        "schemaVersion": "1",
        "sessionId": "session-ordering",
        "serviceEpoch": "epoch-ordering",
        "requestId": request_id,
        "sequence": sequence,
        "messageKind": kind,
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-test",
        "payload": {},
        **fields,
    }


def opened_session(*, initial_sequence: int) -> AspPythonGraphsSession:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-ordering",
            initial_sequence,
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    return session


def test_admitted_message_is_not_claimed_again_by_worker() -> None:
    session = opened_session(initial_sequence=1)
    health = message("health", "health-admitted", 2)

    session.admit_message(health)
    receipt = session.handle_admitted(health)

    assert receipt["messageKind"] == "receipt"
    assert "processId" in receipt["payload"]  # type: ignore[operator]
    assert session._last_sequence == 2


def test_out_of_order_session_sequence_three_then_two_is_rejected() -> None:
    session = opened_session(initial_sequence=3)

    with pytest.raises(ServiceProtocolError) as error:
        session.handle(message("health", "health-sequence-2", 2))

    assert error.value.code == "non-monotonic-sequence"
    assert session._last_sequence == 3


def test_non_hello_service_epoch_drift_does_not_advance_session() -> None:
    session = opened_session(initial_sequence=1)

    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "health",
                "health-epoch-b",
                2,
                serviceEpoch="epoch-b",
            )
        )

    assert error.value.code == "service-epoch-drift"
    assert session.service_epoch == "epoch-ordering"
    assert session._last_sequence == 1
