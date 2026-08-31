"""Validate and construct ASP Server-owned Python graphs session envelopes."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any


ASP_PYTHON_GRAPHS_SCHEMA_ID = "agent.semantic-protocols.asp-python-graphs-session"
ASP_PYTHON_GRAPHS_SCHEMA_VERSION = "1"


@dataclass(frozen=True, slots=True)
class ServiceProtocolError(ValueError):
    code: str
    message: str

    def __str__(self) -> str:
        return self.message


def validate_service_envelope(message: Mapping[str, Any]) -> None:
    if (
        message.get("schemaId") != ASP_PYTHON_GRAPHS_SCHEMA_ID
        or message.get("schemaVersion") != ASP_PYTHON_GRAPHS_SCHEMA_VERSION
    ):
        raise ServiceProtocolError(
            "unsupported-schema", "asp-python-graphs requires session schema v1"
        )
    for field_name in (
        "sessionId",
        "serviceEpoch",
        "requestId",
        "payloadSchemaId",
    ):
        required_string(message, field_name)
    kind = message.get("messageKind")
    if kind not in {
        "hello",
        "open-generation",
        "evaluate",
        "search-evidence",
        "timeline",
        "release-generation",
        "cancel",
        "health",
        "shutdown",
    }:
        raise ServiceProtocolError(
            "unsupported-message-kind", "messageKind is not a supported request kind"
        )
    allowed = {
        "schemaId",
        "schemaVersion",
        "sessionId",
        "serviceEpoch",
        "requestId",
        "clientRequestId",
        "sequence",
        "messageKind",
        "workspaceIdentity",
        "generationDigest",
        "generationToken",
        "runtimeArtifactDigest",
        "executionArtifactDigest",
        "deadlineUnixMillis",
        "cancellationId",
        "payloadSchemaId",
        "payload",
    }
    unknown = sorted(set(message) - allowed)
    if unknown:
        raise ServiceProtocolError(
            "unknown-envelope-field", f"unsupported envelope fields: {unknown}"
        )
    if kind == "cancel":
        required_string(message, "cancellationId")
    if (
        kind == "search-evidence"
        and message.get("payloadSchemaId")
        != "agent.semantic-protocols.asp-python-graphs-search-evidence"
    ):
        raise ServiceProtocolError(
            "invalid-search-evidence-schema",
            "search-evidence requires the canonical search evidence payload schema",
        )
    sequence = message.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or sequence < 1:
        raise ServiceProtocolError(
            "invalid-sequence", "sequence must be a positive integer"
        )


def service_receipt(
    *,
    request_id: str,
    service_epoch: str | None,
    state: str,
    workspace: str | None = None,
    generation: str | None = None,
    generation_token: int | None = None,
    sequence: int = 1,
) -> dict[str, object]:
    receipt: dict[str, object] = {
        "schemaId": ASP_PYTHON_GRAPHS_SCHEMA_ID,
        "schemaVersion": ASP_PYTHON_GRAPHS_SCHEMA_VERSION,
        "sessionId": "asp-python-graphs",
        "serviceEpoch": service_epoch or "unbound",
        "requestId": request_id,
        "sequence": sequence,
        "messageKind": "receipt",
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-receipt",
        "payload": {"state": state},
    }
    if workspace is not None and generation is not None:
        receipt["workspaceIdentity"] = workspace
        receipt["generationDigest"] = generation
        if generation_token is not None:
            receipt["generationToken"] = generation_token
    return receipt


def unavailable_receipt(
    message: Mapping[str, Any] | None, code: str, detail: str
) -> dict[str, object]:
    source = message or {}
    sequence = source.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or sequence < 1:
        sequence = 1
    return {
        "schemaId": ASP_PYTHON_GRAPHS_SCHEMA_ID,
        "schemaVersion": ASP_PYTHON_GRAPHS_SCHEMA_VERSION,
        "sessionId": str(source.get("sessionId") or "asp-python-graphs"),
        "serviceEpoch": str(source.get("serviceEpoch") or "unbound"),
        "requestId": str(source.get("requestId") or "invalid-request"),
        "sequence": sequence,
        "messageKind": "receipt",
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-receipt",
        "payload": {"state": "unavailable", "reasonKind": code, "message": detail},
    }


def required_string(message: Mapping[str, Any], key: str) -> str:
    value = message.get(key)
    if not isinstance(value, str) or not value:
        raise ServiceProtocolError("invalid-identity", f"{key} must be non-empty")
    return value


def required_digest(message: Mapping[str, Any], key: str) -> str:
    value = required_string(message, key)
    digest = value.removeprefix("blake3-256:")
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise ServiceProtocolError(
            "invalid-digest", f"{key} must be a blake3-256 digest"
        )
    return value


def optional_mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def string_sequence(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str) and item)


def string_int_mapping(value: object) -> dict[str, int] | None:
    if not isinstance(value, Mapping):
        return None
    return {
        key: item
        for key, item in value.items()
        if isinstance(key, str)
        and isinstance(item, int)
        and not isinstance(item, bool)
    }


def positive_int(value: object, default: int) -> int:
    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value > 0
        else default
    )


def nonnegative_int(value: object, default: int) -> int:
    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value >= 0
        else default
    )
