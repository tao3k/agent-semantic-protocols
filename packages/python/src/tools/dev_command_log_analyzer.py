"""Summarize semantic dev command logs by session order."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence


@dataclass(frozen=True, slots=True)
class CommandEvent:
    language_id: str
    provider_id: str
    project_root: str
    project_root_hash: str
    method: str
    session_id: str
    session_ordinal: int
    started_at_utc: str
    finished_at_utc: str
    exit_code: int
    elapsed_ms: int
    context_source: str
    event_id: str
    parent_event_id: str | None
    hook_run_id: str | None
    query: str | None
    source_path: Path


def load_command_events(trace_root: Path) -> list[CommandEvent]:
    events: list[CommandEvent] = []
    for path in sorted(trace_root.glob("*/*/commands/*.jsonl")):
        events.extend(_load_jsonl_events(path))
    return sorted(events, key=_event_sort_key)


def render_summary(events: Sequence[CommandEvent]) -> str:
    sessions = _group_by_session(events)
    active = sum(1 for event in events if event.context_source == "active-context")
    fallback = sum(1 for event in events if event.context_source == "project-fallback")
    lines = [
        (
            f"[dev-log-summary] sessions={len(sessions)} commands={len(events)} "
            f"activeContext={active} projectFallback={fallback}"
        )
    ]
    for session_id, session_events in sessions:
        first = session_events[0]
        last = session_events[-1]
        root_hashes = {
            event.project_root_hash for event in session_events if event.project_root_hash
        }
        root_suffix = (
            f" rootHash={next(iter(root_hashes))}"
            if len(root_hashes) == 1
            else f" roots={len(root_hashes)}"
            if root_hashes
            else ""
        )
        lines.append(
            f"|session id={_quote(session_id)} commands={len(session_events)} "
            f"first={first.started_at_utc} last={last.finished_at_utc}{root_suffix}"
        )
        for event in session_events:
            query = f" query={_quote(event.query)}" if event.query else ""
            parent = f" parent={_quote(event.parent_event_id)}" if event.parent_event_id else ""
            hook = f" hook={_quote(event.hook_run_id)}" if event.hook_run_id else ""
            root = (
                f" rootHash={event.project_root_hash}"
                if event.context_source == "project-fallback" and event.project_root_hash
                else ""
            )
            lines.append(
                f"|cmd ordinal={event.session_ordinal} lang={event.language_id} "
                f"provider={event.provider_id} method={event.method} exit={event.exit_code} "
                f"start={event.started_at_utc} elapsedMs={event.elapsed_ms} "
                f"context={event.context_source}"
                f"{root}{parent}{hook}{query}"
            )
    return "\n".join(lines) + "\n"


def dev_command_log_analyzer_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace_root", type=Path)
    args = parser.parse_args(argv)
    events = load_command_events(args.trace_root)
    sys.stdout.write(render_summary(events))
    return 0


def _load_jsonl_events(path: Path) -> list[CommandEvent]:
    events: list[CommandEvent] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
            events.append(_event_from_payload(payload, path))
        except (KeyError, TypeError, ValueError, json.JSONDecodeError):
            continue
    return events


def _event_from_payload(payload: dict[str, Any], path: Path) -> CommandEvent:
    command = _dict(payload["command"])
    result = _dict(payload["result"])
    fields = _dict(payload.get("fields", {}))
    return CommandEvent(
        language_id=str(payload["languageId"]),
        provider_id=str(payload["providerId"]),
        project_root=str(payload.get("projectRoot", "")),
        project_root_hash=str(payload.get("projectRootHash", "")),
        method=str(command["method"]),
        session_id=str(payload["sessionId"]),
        session_ordinal=int(payload["sessionOrdinal"]),
        started_at_utc=str(payload["startedAtUtc"]),
        finished_at_utc=str(payload["finishedAtUtc"]),
        exit_code=int(result["exitCode"]),
        elapsed_ms=int(result["elapsedMs"]),
        context_source=str(fields.get("contextSource", "unknown")),
        event_id=str(payload["eventId"]),
        parent_event_id=_optional_string(payload.get("parentEventId")),
        hook_run_id=_optional_string(payload.get("hookRunId")),
        query=_optional_string(command.get("query")),
        source_path=path,
    )


def _group_by_session(
    events: Sequence[CommandEvent],
) -> list[tuple[str, list[CommandEvent]]]:
    sessions: dict[str, list[CommandEvent]] = {}
    for event in events:
        sessions.setdefault(event.session_id, []).append(event)
    return [
        (session_id, sorted(session_events, key=_event_sort_key))
        for session_id, session_events in sorted(sessions.items())
    ]


def _event_sort_key(event: CommandEvent) -> tuple[str, str, int, str]:
    return (
        event.session_id,
        event.started_at_utc,
        event.session_ordinal,
        event.event_id,
    )


def _dict(value: object) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _optional_string(value: object) -> str | None:
    return value if isinstance(value, str) and value else None


def _quote(value: str | None) -> str:
    if value is None:
        return '""'
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


if __name__ == "__main__":
    raise SystemExit(dev_command_log_analyzer_main())
