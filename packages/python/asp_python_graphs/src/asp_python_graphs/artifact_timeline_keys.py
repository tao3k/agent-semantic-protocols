"""Shared timeline key and event example projections."""

from __future__ import annotations

from .artifact_events import ArtifactEvent


def fanout_key(event: ArtifactEvent) -> tuple[str, str, str, str]:
    subject = event.target or event.query or event.path.rsplit("/", 1)[-1]
    return (event.language, event.method, subject, event.project_root_arg)


def key_row(key: tuple[str, str, str, str]) -> dict[str, str]:
    language, method, subject, project_root_arg = key
    row = {
        "language": language,
        "method": method,
        "subject": subject,
    }
    if project_root_arg:
        row["projectRootArg"] = project_root_arg
    return row


def event_example(event: ArtifactEvent) -> dict[str, object]:
    row = {
        "kind": event.kind,
        "language": event.language,
        "method": event.method,
        "target": event.target or event.query,
        "path": event.path,
    }
    if event.project_root_arg:
        row["projectRootArg"] = event.project_root_arg
    return row
