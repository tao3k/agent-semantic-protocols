# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Exercise bounded gRPC envelope and shared-session behavior."""

from __future__ import annotations

import asyncio
import time
from collections.abc import AsyncIterator

import pytest

from asp_python_graphs.grpc_service import (
    SERVICE_NAME,
    SESSION_METHOD,
    AspPythonGraphsGrpcHandler,
    decode_envelope,
    encode_envelope,
)
from service_session_support import (
    DIGEST_B,
    generation_payload,
    resident_evaluation_payload,
)


def health(request_id: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.asp-python-graphs-session",
        "schemaVersion": "1",
        "sessionId": "session-a",
        "serviceEpoch": "epoch-a",
        "requestId": request_id,
        "sequence": 1,
        "messageKind": "health",
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-health",
        "payload": {},
    }


def test_json_envelope_round_trips_through_protobuf_bytes_field() -> None:
    packet = health("health-1")
    assert decode_envelope(encode_envelope(packet)) == packet


def test_grpc_namespace_is_stable_and_schema_owns_versioning() -> None:
    assert SERVICE_NAME == "asp.python.graphs.AspPythonGraphs"
    assert SESSION_METHOD == "/asp.python.graphs.AspPythonGraphs/Session"
    assert ".v1" not in SERVICE_NAME
    assert health("health-version-owner")["schemaVersion"] == "1"


@pytest.mark.parametrize("encoded", [b"", b"{}", b"\x0a\x05{}"])
def test_malformed_protobuf_envelope_is_rejected(encoded: bytes) -> None:
    with pytest.raises((ValueError, UnicodeDecodeError)):
        decode_envelope(encoded)


def test_handler_rejects_unbounded_capacity() -> None:
    with pytest.raises(ValueError, match="positive"):
        AspPythonGraphsGrpcHandler(max_in_flight=0)


def test_concurrent_health_requests_share_one_service_session() -> None:
    async def requests() -> AsyncIterator[dict[str, object]]:
        yield {
            **health("hello-1"),
            "messageKind": "hello",
            "runtimeArtifactDigest": f"blake3-256:{'a' * 64}",
            "executionArtifactDigest": f"blake3-256:{'b' * 64}",
        }
        for index in range(8):
            yield health(f"health-{index}")

    async def collect() -> list[dict[str, object]]:
        handler = AspPythonGraphsGrpcHandler(max_in_flight=2)
        return [receipt async for receipt in handler.session(requests(), object())]  # type: ignore[arg-type]

    receipts = asyncio.run(collect())
    assert len(receipts) == 9
    assert {str(receipt["serviceEpoch"]) for receipt in receipts} == {"epoch-a"}
    assert all(receipt["messageKind"] == "receipt" for receipt in receipts)


def test_removed_compile_generation_returns_typed_failures() -> None:
    async def requests() -> AsyncIterator[dict[str, object]]:
        for index in range(33):
            yield {
                **health(f"compile-{index}"),
                "sequence": index + 1,
                "messageKind": "compile-generation",
                "workspaceIdentity": "workspace-a",
                "generationDigest": f"blake3-256:{'a' * 64}",
                "payload": {"graph": {"nodes": [], "edges": []}},
            }

    async def collect() -> list[dict[str, object]]:
        handler = AspPythonGraphsGrpcHandler(max_in_flight=32)
        return [receipt async for receipt in handler.session(requests(), object())]  # type: ignore[arg-type]

    receipts = asyncio.run(collect())
    assert len(receipts) == 33
    assert all(
        receipt.get("payload", {}).get("reasonKind") == "unsupported-message-kind"  # type: ignore[union-attr]
        for receipt in receipts
    )


def test_concurrent_resident_rank_can_be_cancelled_without_late_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from asp_python_graphs import algorithm

    original_rank_graph = algorithm.rank_graph

    def slow_rank_graph(*args: object, **kwargs: object):  # type: ignore[no-untyped-def]
        time.sleep(0.05)
        return original_rank_graph(*args, **kwargs)

    monkeypatch.setattr(algorithm, "rank_graph", slow_rank_graph)

    async def requests() -> AsyncIterator[dict[str, object]]:
        yield {
            **health("hello-1"),
            "messageKind": "hello",
            "runtimeArtifactDigest": f"blake3-256:{'a' * 64}",
            "executionArtifactDigest": DIGEST_B,
        }
        yield {
            **health("generation-1"),
            "sequence": 2,
            "messageKind": "generation-graph",
            "workspaceIdentity": "workspace-test",
            "generationDigest": DIGEST_B,
            "payload": generation_payload(),
        }
        yield {
            **health("rank-1"),
            "sequence": 3,
            "messageKind": "evaluate-resident",
            "workspaceIdentity": "workspace-test",
            "generationDigest": DIGEST_B,
            "payload": resident_evaluation_payload(),
        }
        yield {
            **health("cancel-rank-1"),
            "sequence": 4,
            "messageKind": "cancel",
            "cancellationId": "rank-1",
        }

    async def collect() -> list[dict[str, object]]:
        handler = AspPythonGraphsGrpcHandler(max_in_flight=2)
        return [receipt async for receipt in handler.session(requests(), object())]  # type: ignore[arg-type]

    receipts = asyncio.run(collect())
    by_request = {str(receipt["requestId"]): receipt for receipt in receipts}
    assert by_request["cancel-rank-1"]["payload"]["state"] == "cancelled"  # type: ignore[index]
    if "rank-1" in by_request:
        assert by_request["rank-1"]["payload"]["state"] == "cancelled"  # type: ignore[index]
        assert "result" not in by_request["rank-1"]["payload"]  # type: ignore[operator]
