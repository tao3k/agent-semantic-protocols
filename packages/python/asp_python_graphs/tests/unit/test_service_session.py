"""Exercise ASP Python Graphs generation ownership and evaluation identity."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, message, rank_payload


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
