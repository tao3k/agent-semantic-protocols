"""Exercise generation-bound rg, lexical, and Python graph evidence fan-in."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, message, rank_payload


SEARCH_EVIDENCE_SCHEMA = "agent.semantic-protocols.asp-python-graphs-search-evidence"


def _graph() -> object:
    return rank_payload()["graph"]


def _message(
    kind: str, request_id: str, sequence: int, **fields: object
) -> dict[str, object]:
    return message(kind, request_id, sequence=sequence, **fields)


def _opened_session() -> AspPythonGraphsSession:
    session = AspPythonGraphsSession()
    session.handle(
        _message(
            "hello",
            "hello-1",
            1,
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    session.handle(
        _message(
            "open-generation",
            "open-1",
            2,
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={"graph": _graph()},
        )
    )
    return session


def _evidence(
    session: AspPythonGraphsSession,
    request_id: str,
    sequence: int,
    lane: str,
    owners: list[str],
    elapsed_micros: int,
    *,
    frame_id: str | None = None,
) -> dict[str, object]:
    return session.handle(
        _message(
            "search-evidence",
            request_id,
            sequence,
            payloadSchemaId=SEARCH_EVIDENCE_SCHEMA,
            workspaceIdentity="workspace-a",
            generationDigest=DIGEST_A,
            payload={
                "frameId": frame_id or request_id,
                "intent": "conceptual",
                "lane": lane,
                "ownerIds": owners,
                "complete": True,
                "elapsedMicros": elapsed_micros,
            },
        )
    )


def test_search_evidence_frames_incrementally_guide_the_shared_search_route() -> None:
    session = _opened_session()
    lexical = _evidence(
        session, "search-lexical", 3, "indexed-lexical", ["owner-a", "owner-b"], 80
    )
    graph = _evidence(
        session, "search-graph", 4, "python-graph", ["owner-a", "owner-c"], 120
    )
    verified = _evidence(session, "search-rg", 5, "ripgrep", ["owner-a", "owner-c"], 40)

    assert lexical["payload"]["recommendedNext"] == "submit-python-graph"  # type: ignore[index]
    assert graph["payload"]["recommendedNext"] == "verify-candidates"  # type: ignore[index]
    assert verified["payload"]["recommendedNext"] == "stop-evidence-sufficient"  # type: ignore[index]
    assert verified["payload"]["correlation"] == {  # type: ignore[index]
        "lexicalGraphOverlapCount": 1,
        "graphMarginalCandidateCount": 1,
        "verifiedUnionCandidateCount": 2,
    }


def test_search_evidence_frame_is_idempotent_and_generation_bound() -> None:
    session = _opened_session()
    _evidence(
        session,
        "search-1",
        3,
        "indexed-lexical",
        ["owner-a"],
        12,
        frame_id="lexical-1",
    )
    duplicate = _evidence(
        session,
        "search-2",
        4,
        "indexed-lexical",
        ["owner-a"],
        12,
        frame_id="lexical-1",
    )
    assert duplicate["payload"]["duplicate"] is True  # type: ignore[index]
    assert duplicate["payload"]["frameCount"] == 1  # type: ignore[index]

    with pytest.raises(ServiceProtocolError, match="open-generation"):
        session.handle(
            _message(
                "search-evidence",
                "search-stale",
                5,
                payloadSchemaId=SEARCH_EVIDENCE_SCHEMA,
                workspaceIdentity="workspace-a",
                generationDigest=DIGEST_B,
                payload={
                    "frameId": "stale-1",
                    "intent": "conceptual",
                    "lane": "indexed-lexical",
                    "ownerIds": ["owner-a"],
                    "complete": True,
                    "elapsedMicros": 12,
                },
            )
        )
