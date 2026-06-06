"""Run a command and record it as a trace event."""

from __future__ import annotations

import json
import shlex
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class TraceRecordConfig:
    trace_root: Path
    session_id: str
    language_id: str
    provider_id: str
    cwd: Path


def record_command(argv: list[str], *, config: TraceRecordConfig) -> dict[str, Any]:
    started = time.time()
    started_at = _utc_timestamp(started)
    completed = subprocess.run(
        argv,
        cwd=config.cwd,
        capture_output=True,
        text=False,
        check=False,
    )
    finished = time.time()
    event_id = _event_id(config.session_id, finished)
    stdout_path = _write_output(config.trace_root, event_id, "stdout", completed.stdout)
    stderr_path = _write_output(config.trace_root, event_id, "stderr", completed.stderr)
    next_items = _hot_block_selectors(completed.stdout)
    event = {
        "schemaId": "agent.semantic-protocols.dev-command-log",
        "schemaVersion": "1",
        "eventId": event_id,
        "sessionId": config.session_id,
        "languageId": config.language_id,
        "providerId": config.provider_id,
        "argv": argv,
        "cwd": str(config.cwd),
        "startedAtUtc": started_at,
        "finishedAtUtc": _utc_timestamp(finished),
        "command": {
            "method": _method(argv),
        },
        "result": {
            "exitCode": completed.returncode,
            "elapsedMs": max(0, int((finished - started) * 1000)),
            "stdoutBytes": len(completed.stdout),
            "stderrBytes": len(completed.stderr),
            "stdoutPath": str(stdout_path),
            "stderrPath": str(stderr_path),
            "status": "success" if completed.returncode == 0 else "fail",
        },
    }
    if next_items:
        event["next"] = next_items
    _append_event(config.trace_root, config.language_id, config.provider_id, event)
    return event


def _append_event(
    trace_root: Path,
    language_id: str,
    provider_id: str,
    event: dict[str, Any],
) -> None:
    path = (
        trace_root
        / language_id
        / provider_id
        / "commands"
        / f"{event['eventId']}.jsonl"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(event, sort_keys=True, separators=(",", ":")))
        handle.write("\n")


def _write_output(trace_root: Path, event_id: str, suffix: str, content: bytes) -> Path:
    path = Path("outputs") / f"{event_id}.{suffix}"
    output_path = trace_root / path
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(content)
    return path


def _hot_block_selectors(stdout: bytes) -> list[str]:
    selectors: list[str] = []
    text = stdout.decode("utf-8", errors="replace")
    for line in text.splitlines():
        if not line.startswith("|hotBlock "):
            continue
        selector = _line_field(line, "selector")
        if selector:
            selectors.append(selector)
    return selectors


def _line_field(line: str, field: str) -> str:
    prefix = f"{field}="
    for part in shlex.split(line):
        if part.startswith(prefix):
            return part.removeprefix(prefix)
    return ""


def _event_id(session_id: str, timestamp: float) -> str:
    safe_session = "".join(
        character if character.isalnum() or character in "-_." else "-"
        for character in session_id
    ).strip("-")
    millis = int(timestamp * 1000)
    return f"trace-record-{safe_session or 'session'}-{millis}"


def _method(argv: list[str]) -> str:
    if len(argv) >= 3 and argv[0] == "asp":
        return "/".join(argv[2:4]) if len(argv) >= 4 else argv[2]
    if len(argv) >= 2:
        return "/".join(argv[:2])
    return argv[0] if argv else "unknown"


def _utc_timestamp(timestamp: float) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(timestamp))
