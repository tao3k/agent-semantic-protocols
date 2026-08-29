"""Exercise bounded gRPC envelope and shared-session behavior."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator

import pytest

from asp_python_graphs.grpc_service import (
    SERVICE_NAME,
    SESSION_METHOD,
    AspPythonGraphsGrpcHandler,
    decode_envelope,
    encode_envelope,
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


def test_evaluate_admission_reports_deterministic_capacity_saturation() -> None:
    async def requests() -> AsyncIterator[dict[str, object]]:
        for index in range(33):
            yield {
                **health(f"evaluate-{index}"),
                "sequence": index + 1,
                "messageKind": "evaluate",
                "workspaceIdentity": "workspace-a",
                "generationDigest": f"blake3-256:{'a' * 64}",
                "generationToken": 1,
                "payload": {"terms": ["runtime"], "rankPayload": {}},
            }

    async def collect() -> list[dict[str, object]]:
        handler = AspPythonGraphsGrpcHandler(max_in_flight=32)
        return [receipt async for receipt in handler.session(requests(), object())]  # type: ignore[arg-type]

    receipts = asyncio.run(collect())
    assert len(receipts) == 33
    assert sum(
        receipt.get("payload", {}).get("reasonKind") == "capacity-exhausted"  # type: ignore[union-attr]
        for receipt in receipts
    ) == 1
