"""CLI adapter for listing trace sessions."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

from .output import emit, emit_json
from .report_format import quote_value
from .trace_sessions import list_trace_sessions
from .utils import dict_value, resolve_path, string_list


def add_trace_session_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--list-trace-sessions",
        metavar="TRACE_ROOT",
        help="List session ids discovered in a command trace root or JSONL file.",
    )


def handle_trace_session_args(repo_root: Path, args: Any) -> int | None:
    trace_arg = getattr(args, "list_trace_sessions", None)
    if not trace_arg:
        return None
    trace_path = resolve_path(repo_root, trace_arg) or (repo_root / trace_arg).resolve()
    report = list_trace_sessions(
        trace_path,
        language_id=args.trace_language_id,
        provider_id=args.trace_provider_id,
    )
    if args.json:
        emit_json(report)
    else:
        print_trace_session_report(report)
    return 0


def print_trace_session_report(report: dict[str, Any]) -> None:
    summary = dict_value(report.get("summary"))
    emit(
        "[trace-sessions] "
        f"sessions={summary.get('sessionCount', 0)} "
        f"commands={summary.get('commandCount', 0)} "
        f"files={summary.get('traceFileCount', 0)}"
    )
    sessions = report.get("sessions")
    if not isinstance(sessions, list):
        return
    for session in sessions:
        if isinstance(session, dict):
            _print_session(session)


def _print_session(session: dict[str, Any]) -> None:
    emit(
        "|session "
        f"id={quote_value(str(session.get('sessionId', 'unknown')))} "
        f"commands={session.get('commandCount', 0)} "
        f"languages={_joined(session.get('languages'))} "
        f"providers={_joined(session.get('providers'))} "
        f"elapsedMs={session.get('elapsedMs', 0)} "
        f"stdoutBytes={session.get('stdoutBytes', 0)} "
        f"first={quote_value(str(session.get('firstStartedAtUtc') or 'unknown'))} "
        f"last={quote_value(str(session.get('lastFinishedAtUtc') or 'unknown'))}"
    )


def _joined(value: Any) -> str:
    items = string_list(value)
    return ",".join(items) if items else "unknown"
