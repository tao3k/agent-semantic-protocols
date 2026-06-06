"""Summarize sessions from recorded command trace roots."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .trace_receipt_events import TraceCommandFilter, TraceCommandParser
from .utils import dict_value, optional_int


@dataclass
class TraceSessionAccumulator:
    session_id: str
    command_count: int = 0
    elapsed_ms: int = 0
    stdout_bytes: int = 0
    stderr_bytes: int = 0
    first_started_at_utc: str | None = None
    last_finished_at_utc: str | None = None
    languages: set[str] = field(default_factory=set)
    providers: set[str] = field(default_factory=set)
    project_root_hashes: set[str] = field(default_factory=set)
    trace_files: set[str] = field(default_factory=set)

    def add(self, payload: dict[str, Any], path: Path) -> None:
        self.command_count += 1
        self.trace_files.add(str(path))
        self._add_optional("languageId", payload, self.languages)
        self._add_optional("providerId", payload, self.providers)
        self._add_optional("projectRootHash", payload, self.project_root_hashes)
        self.elapsed_ms += _metric(payload, "elapsedMs")
        self.stdout_bytes += _metric(payload, "stdoutBytes")
        self.stderr_bytes += _metric(payload, "stderrBytes")
        self._record_time(payload)

    def to_dict(self) -> dict[str, Any]:
        return {
            "sessionId": self.session_id,
            "commandCount": self.command_count,
            "elapsedMs": self.elapsed_ms,
            "stdoutBytes": self.stdout_bytes,
            "stderrBytes": self.stderr_bytes,
            "firstStartedAtUtc": self.first_started_at_utc,
            "lastFinishedAtUtc": self.last_finished_at_utc,
            "languages": sorted(self.languages),
            "providers": sorted(self.providers),
            "projectRootHashes": sorted(self.project_root_hashes),
            "traceFileCount": len(self.trace_files),
        }

    def _add_optional(
        self,
        field_name: str,
        payload: dict[str, Any],
        target: set[str],
    ) -> None:
        value = payload.get(field_name)
        if isinstance(value, str) and value:
            target.add(value)

    def _record_time(self, payload: dict[str, Any]) -> None:
        started_at = _optional_str(payload.get("startedAtUtc")) or _optional_str(
            payload.get("timestampUtc")
        )
        finished_at = _optional_str(payload.get("finishedAtUtc")) or _optional_str(
            payload.get("timestampUtc")
        )
        if started_at and (
            self.first_started_at_utc is None or started_at < self.first_started_at_utc
        ):
            self.first_started_at_utc = started_at
        if finished_at and (
            self.last_finished_at_utc is None or finished_at > self.last_finished_at_utc
        ):
            self.last_finished_at_utc = finished_at


def list_trace_sessions(
    trace_path: Path,
    *,
    language_id: str | None = None,
    provider_id: str | None = None,
) -> dict[str, Any]:
    parser = TraceCommandParser(
        filters=TraceCommandFilter(language_id=language_id, provider_id=provider_id)
    )
    sessions: dict[str, TraceSessionAccumulator] = {}
    file_count = 0
    for path in parser.trace_files(trace_path):
        file_count += 1
        for payload in _json_payloads(path):
            if not parser.payload_matches_filters(payload):
                continue
            session_id = _optional_str(payload.get("sessionId"))
            if session_id is None:
                continue
            sessions.setdefault(
                session_id,
                TraceSessionAccumulator(session_id),
            ).add(payload, path)
    session_rows = [session.to_dict() for session in sessions.values()]
    session_rows.sort(
        key=lambda row: (
            str(row.get("lastFinishedAtUtc") or ""),
            str(row.get("sessionId") or ""),
        ),
        reverse=True,
    )
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-trace-sessions",
        "schemaVersion": "1",
        "summary": {
            "sessionCount": len(session_rows),
            "commandCount": sum(
                int(row.get("commandCount") or 0) for row in session_rows
            ),
            "traceFileCount": file_count,
        },
        "sessions": session_rows,
    }


def _json_payloads(path: Path) -> list[dict[str, Any]]:
    payloads: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            try:
                payload = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(payload, dict):
                payloads.append(payload)
    return payloads


def _metric(payload: dict[str, Any], field_name: str) -> int:
    result = dict_value(payload.get("result"))
    result_value = optional_int(result.get(field_name))
    if result_value is not None:
        return result_value
    metrics = dict_value(payload.get("metrics"))
    return optional_int(metrics.get(field_name)) or 0


def _optional_str(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None
