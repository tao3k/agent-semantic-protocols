"""Artifact event extraction for graph turbo timeline audits."""

from __future__ import annotations

import json
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from .artifact_event_model import ArtifactEvent
from .artifact_event_packet import (
    artifact_events_from_packet as artifact_events_from_packet,
    artifact_events_packet as artifact_events_packet,
)
from .artifact_event_commands import (
    artifact_command_argv,
    artifact_command_method,
    artifact_command_query,
    artifact_command_target,
)
from .artifact_project_roots import (
    artifact_infer_project_root,
    artifact_project_root_arg,
    artifact_workspace_root,
)
from .artifact_topology import packet_topology_metadata


def scan_artifact_events(root: Path) -> tuple[ArtifactEvent, ...]:
    workspace_root_path = artifact_workspace_root(root)
    events = [
        event
        for directory in _artifact_dirs(root)
        for event in _events_from_directory(directory, workspace_root_path)
    ]
    return tuple(sorted(events, key=lambda event: (event.timestamp, event.path)))


def _artifact_dirs(root: Path) -> tuple[Path, ...]:
    if root.name in _KNOWN_DIRS:
        return (root,)
    return tuple(root / name for name in _KNOWN_DIRS if (root / name).is_dir())


def _events_from_directory(
    directory: Path, workspace_root: Path
) -> Iterable[ArtifactEvent]:
    for path in sorted(directory.iterdir()):
        if not path.is_file():
            continue
        if directory.name == "prompt-output":
            yield from _prompt_output_events(path, workspace_root)
        elif directory.name == "search":
            yield from _packet_event(path, "search", workspace_root)
        elif directory.name == "query":
            yield from _packet_event(path, "query", workspace_root)
        elif directory.name == "semantic-tree-sitter-query":
            yield from _packet_event(path, "tree-sitter-query", workspace_root)
        elif directory.name == "search-output":
            yield _text_event(path, "search-output", workspace_root)
        elif directory.name == "analysis-metadata":
            yield from _packet_event(path, "analysis-metadata", workspace_root)


def _prompt_output_events(path: Path, workspace_root: Path) -> Iterable[ArtifactEvent]:
    if path.name.endswith(".command.json"):
        yield from _command_events(path, workspace_root)
    elif path.suffix == ".txt":
        yield _text_event(path, "prompt-output", workspace_root)


def _packet_event(
    path: Path, kind: str, workspace_root: Path
) -> Iterable[ArtifactEvent]:
    if path.suffix != ".json":
        return ()
    packet = _load_json(path)
    target = _packet_target(packet)
    query = str(packet.get("query") or "")
    project_root = str(packet.get("projectRoot") or "") or artifact_infer_project_root(
        workspace_root,
        target or query,
    )
    return (
        _event(
            path,
            kind,
            language=str(packet.get("languageId") or _language_from_name(path)),
            method=str(packet.get("method") or _method_from_name(path)),
            target=target,
            query=query,
            project_root=project_root,
            workspace_root=workspace_root,
            metadata=packet_topology_metadata(packet),
        ),
    )


def _command_events(path: Path, workspace_root: Path) -> Iterable[ArtifactEvent]:
    packet = _load_json(path)
    commands = packet.get("providerCommands")
    if not isinstance(commands, list):
        return ()
    events: list[ArtifactEvent] = []
    for command in commands:
        if not isinstance(command, Mapping):
            continue
        argv = artifact_command_argv(command.get("argv"))
        target = artifact_command_target(argv)
        query = artifact_command_query(argv)
        project_root = str(
            command.get("projectRoot") or ""
        ) or artifact_infer_project_root(
            workspace_root,
            target or query,
        )
        events.append(
            _event(
                path,
                "command",
                language=str(command.get("languageId") or _language_from_name(path)),
                method=artifact_command_method(argv),
                target=target,
                query=query,
                project_root=project_root,
                workspace_root=workspace_root,
                argv=argv,
            )
        )
    return tuple(events)


def _text_event(path: Path, kind: str, workspace_root: Path) -> ArtifactEvent:
    return _event(
        path,
        kind,
        language=_language_from_name(path),
        method=_method_from_name(path),
        target="",
        query="",
        project_root="",
        workspace_root=workspace_root,
    )


def _event(
    path: Path,
    kind: str,
    *,
    language: str,
    method: str,
    target: str,
    query: str,
    project_root: str,
    workspace_root: Path,
    argv: tuple[str, ...] = (),
    metadata: Mapping[str, object] | None = None,
) -> ArtifactEvent:
    stat = path.stat()
    return ArtifactEvent(
        timestamp=stat.st_mtime,
        kind=kind,
        language=language,
        method=method,
        target=target,
        query=query,
        project_root=project_root,
        project_root_arg=artifact_project_root_arg(project_root, workspace_root),
        path=str(path),
        bytes=stat.st_size,
        argv=argv,
        metadata=dict(metadata or {}),
    )


def _load_json(path: Path) -> Mapping[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    return value if isinstance(value, Mapping) else {}


def _packet_target(packet: Mapping[str, Any]) -> str:
    owner = packet.get("ownerPath")
    if isinstance(owner, str):
        return owner
    owners = packet.get("owners")
    if isinstance(owners, list) and owners:
        first = owners[0]
        if isinstance(first, Mapping) and isinstance(first.get("path"), str):
            return str(first["path"])
    return ""


def _language_from_name(path: Path) -> str:
    return path.name.split("-", 1)[0] if "-" in path.name else "unknown"


def _method_from_name(path: Path) -> str:
    parts = path.stem.removesuffix(".command").split("-")
    if len(parts) < 3:
        return "unknown"
    return "/".join(parts[1:-1])


_KNOWN_DIRS = (
    "analysis-metadata",
    "prompt-output",
    "query",
    "search",
    "search-output",
    "semantic-tree-sitter-query",
)
