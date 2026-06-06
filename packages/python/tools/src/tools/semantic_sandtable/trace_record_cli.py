"""CLI adapter for recording command traces."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from .output import emit, emit_json
from .trace_record import TraceRecordConfig, record_command
from .utils import dict_value, resolve_path


def add_trace_record_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--record-trace-root",
        metavar="TRACE_ROOT",
        help="Trace root where --record-command writes a command event.",
    )
    parser.add_argument(
        "--record-session-id",
        help="Session id for --record-command.",
    )
    parser.add_argument(
        "--record-command",
        nargs=argparse.REMAINDER,
        help="Command argv to execute and record. Put this option last.",
    )


def handle_trace_record_args(repo_root: Path, args: Any) -> int | None:
    command = getattr(args, "record_command", None)
    trace_root_arg = getattr(args, "record_trace_root", None)
    if not command and not trace_root_arg:
        return None
    if not command or command == ["--"]:
        emit("--record-command requires command argv", file=sys.stderr)
        return 2
    if command and command[0] == "--":
        command = command[1:]
    trace_root = resolve_path(repo_root, trace_root_arg or "") or repo_root
    session_id = args.record_session_id or args.trace_session_id or "recorded-session"
    event = record_command(
        command,
        config=TraceRecordConfig(
            trace_root=trace_root,
            session_id=session_id,
            language_id=args.trace_language_id or args.language,
            provider_id=args.trace_provider_id or "unknown",
            cwd=repo_root,
        ),
    )
    if args.json:
        emit_json(event)
    else:
        _print_record_event(event)
    return int(dict_value(event.get("result")).get("exitCode") or 0)


def _print_record_event(event: dict[str, Any]) -> None:
    result = dict_value(event.get("result"))
    emit(
        "[trace-record] "
        f"event={event.get('eventId', 'unknown')} "
        f"session={event.get('sessionId', 'unknown')} "
        f"exit={result.get('exitCode', 'unknown')} "
        f"elapsedMs={result.get('elapsedMs', 0)} "
        f"stdoutBytes={result.get('stdoutBytes', 0)} "
        f"stderrBytes={result.get('stderrBytes', 0)}"
    )
