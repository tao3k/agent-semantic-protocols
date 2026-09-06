# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Verify the Search-owned generation graph operation and typed receipt."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, generation_payload, message


def _session() -> AspPythonGraphsSession:
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


def _payload() -> dict[str, object]:
    return generation_payload()


def test_generation_graph_returns_content_bound_owner_frontier() -> None:
    receipt = _session().handle(
        message(
            "generation-graph",
            "generation-1",
            sequence=2,
            workspaceIdentity="workspace-test",
            generationDigest=DIGEST_B,
            payload=_payload(),
        )
    )
    result = receipt["payload"]["result"]  # type: ignore[index]
    assert (
        result["schemaId"] == "agent.semantic-protocols.search-generation-graph-receipt"
    )
    assert result["entryOwnerIds"] == ["src/a.py", "src/b.py"]
    assert result["entryNodeIds"] == ["owner:src/a.py", "owner:src/b.py"]
    assert result["candidateOwnerIds"] == result["entryOwnerIds"]
    assert result["artifactDigest"].startswith("blake3-256:")
    assert result["complete"] is True


def test_generation_graph_rejects_cross_owner_relation() -> None:
    payload = _payload()
    payload["relations"] = [
        {
            "from": {"kind": "owner", "id": "src/a.py"},
            "kind": "imports",
            "to": {"kind": "owner", "id": "src/missing.py"},
        }
    ]
    with pytest.raises(ServiceProtocolError) as error:
        _session().handle(
            message(
                "generation-graph",
                "generation-1",
                sequence=2,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_B,
                payload=payload,
            )
        )
    assert error.value.code == "invalid-generation-graph"


def test_generation_graph_rejects_unknown_request_fields() -> None:
    payload = _payload()
    payload["legacySeeds"] = []
    with pytest.raises(ServiceProtocolError) as error:
        _session().handle(
            message(
                "generation-graph",
                "generation-1",
                sequence=2,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_B,
                payload=payload,
            )
        )
    assert error.value.code == "invalid-generation-graph"


def test_generation_graph_rejects_partial_nested_identity_evidence() -> None:
    payload = _payload()
    payload["sourceSnapshot"] = {"rootDigest": DIGEST_A}
    with pytest.raises(ServiceProtocolError) as error:
        _session().handle(
            message(
                "generation-graph",
                "generation-1",
                sequence=2,
                workspaceIdentity="workspace-test",
                generationDigest=DIGEST_B,
                payload=payload,
            )
        )
    assert error.value.code == "invalid-generation-graph"
