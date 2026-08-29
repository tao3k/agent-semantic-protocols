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
        "evaluate-1": 3,
        "evaluate-stale": 2,
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
    packet["schemaId"] = "agent.semantic-protocols.graph-turbo-resident-server"
    with pytest.raises(ServiceProtocolError) as error:
        AspPythonGraphsSession().handle(packet)
    assert error.value.code == "unsupported-schema"
