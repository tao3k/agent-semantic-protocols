"""Artifact event packet serialization for graph turbo timeline audits."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from .artifact_event_model import ArtifactEvent


def artifact_events_packet(
    artifact_dir: Path,
    events: Iterable[ArtifactEvent],
    *,
    source_kind: str,
    db_path: str = "",
) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.graph-turbo-artifact-events",
        "schemaVersion": "1",
        "artifactDir": str(artifact_dir),
        "source": {
            "kind": source_kind,
            "dbPath": db_path,
        },
        "events": [_event_row(event) for event in events],
    }


def artifact_events_from_packet(packet: Mapping[str, Any]) -> tuple[ArtifactEvent, ...]:
    if packet.get("schemaId") != "agent.semantic-protocols.graph-turbo-artifact-events":
        raise ValueError("artifact events packet has unsupported schemaId")
    events = packet.get("events")
    if not isinstance(events, list):
        raise ValueError("artifact events packet events must be an array")
    return tuple(
        sorted(
            (_event_from_row(row) for row in events if isinstance(row, Mapping)),
            key=lambda event: (event.timestamp, event.path),
        )
    )


def _event_row(event: ArtifactEvent) -> dict[str, object]:
    row: dict[str, object] = {
        "timestamp": event.timestamp,
        "kind": event.kind,
        "language": event.language,
        "method": event.method,
        "target": event.target,
        "query": event.query,
        "projectRoot": event.project_root,
        "projectRootArg": event.project_root_arg,
        "path": event.path,
        "bytes": event.bytes,
    }
    if event.metadata:
        row["metadata"] = dict(event.metadata)
    return row


def _event_from_row(row: Mapping[str, Any]) -> ArtifactEvent:
    return ArtifactEvent(
        timestamp=_number_field(row, "timestamp"),
        kind=_string_field(row, "kind"),
        language=_string_field(row, "language"),
        method=_string_field(row, "method"),
        target=_string_field(row, "target"),
        query=_string_field(row, "query"),
        project_root=_string_field(row, "projectRoot"),
        project_root_arg=_string_field(row, "projectRootArg"),
        path=_string_field(row, "path"),
        bytes=int(_number_field(row, "bytes")),
        metadata=_metadata_field(row, "metadata"),
    )


def _string_field(row: Mapping[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str):
        raise ValueError(f"artifact event field {key} must be a string")
    return value


def _number_field(row: Mapping[str, Any], key: str) -> float:
    value = row.get(key)
    if not isinstance(value, int | float):
        raise ValueError(f"artifact event field {key} must be numeric")
    return float(value)


def _metadata_field(row: Mapping[str, Any], key: str) -> dict[str, object]:
    value = row.get(key)
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ValueError(f"artifact event field {key} must be an object")
    return {str(field): item for field, item in value.items()}
