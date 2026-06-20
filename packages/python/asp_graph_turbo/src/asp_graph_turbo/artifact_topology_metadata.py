"""Topology metadata extraction from cached ASP artifact packets."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import replace
from pathlib import Path
from typing import Any

from .artifact_event_model import ArtifactEvent

TOPOLOGY_EVENT_KINDS = {
    "analysis-metadata",
    "query",
    "search",
    "tree-sitter-query",
}

PACKET_TOPOLOGY_KEYS = (
    "actionFrontier",
    "actionRank",
    "clauseCoverage",
    "commandHandles",
    "declarationCoverage",
    "evidenceFrontier",
    "globalCoverage",
    "nextClasses",
    "nextCommand",
    "ownerCandidates",
    "ownerCoverage",
    "packageClusters",
    "packageCohesion",
    "parserIndexNext",
    "pathCoverage",
    "queryPack",
    "queryQuality",
    "rankedEvidence",
    "recommendedNext",
    "rgScopeNext",
    "risk",
    "scopeQuality",
    "seedPlan",
    "seedPlanDetail",
    "sourceTrace",
)


def packet_topology_metadata(packet: Mapping[str, Any]) -> dict[str, object]:
    metadata = {
        key: _json_safe(packet[key])
        for key in PACKET_TOPOLOGY_KEYS
        if key in packet
    }
    return {key: value for key, value in metadata.items() if value != ""}


def hydrate_topology_metadata(
    events: tuple[ArtifactEvent, ...], artifact_dir: Path
) -> tuple[ArtifactEvent, ...]:
    hydrated: list[ArtifactEvent] = []
    for event in events:
        if event.metadata or event.kind not in TOPOLOGY_EVENT_KINDS:
            hydrated.append(event)
            continue
        metadata = _metadata_from_artifact(event, artifact_dir)
        hydrated.append(replace(event, metadata=metadata) if metadata else event)
    return tuple(hydrated)


def _metadata_from_artifact(
    event: ArtifactEvent, artifact_dir: Path
) -> dict[str, object]:
    path = Path(event.path)
    if not path.is_absolute():
        path = artifact_dir / path
    if path.suffix != ".json" or not path.is_file():
        return {}
    try:
        packet = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    if not isinstance(packet, Mapping):
        return {}
    return packet_topology_metadata(packet)


def _json_safe(value: object, *, depth: int = 0) -> object:
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if depth >= 3:
        return str(value)
    if isinstance(value, Mapping):
        return {
            str(key): _json_safe(item, depth=depth + 1)
            for key, item in list(value.items())[:32]
        }
    if isinstance(value, Sequence):
        return [_json_safe(item, depth=depth + 1) for item in list(value)[:32]]
    return str(value)
