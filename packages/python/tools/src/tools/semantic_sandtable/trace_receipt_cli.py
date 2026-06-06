"""CLI adapter for building receipts from command traces."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from .output import emit, emit_json
from .receipt_reports import print_receipt_report
from .receipts import validate_receipt_path
from .reports import receipt_report_json
from .trace_receipt_events import TraceCommandFilter
from .trace_receipts import (
    TraceReceiptConfig,
    build_receipt_from_trace_path,
    write_receipt_from_trace_path,
)
from .utils import resolve_path


def add_trace_receipt_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--build-receipt-from-trace",
        metavar="TRACE",
        help="Build a sandtable receipt JSON document from a command trace file.",
    )
    parser.add_argument(
        "--output",
        help=(
            "Output path for --build-receipt-from-trace. "
            "If omitted, the receipt JSON is printed to stdout."
        ),
    )
    parser.add_argument(
        "--scenario-id",
        default="recorded.agent-trace",
        help="Scenario id for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--language",
        default="unknown",
        help="Language id for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--project-name",
        default="unknown",
        help="Project name for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--project-source",
        default="checkout",
        choices=("checkout", "registry", "fixture", "unknown"),
        help="Project source for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--intent",
        default="Record agent command trace for sandtable receipt validation.",
        help="Intent text for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--edit-boundary",
        default="before-edit",
        choices=("before-edit", "after-edit"),
        help="Edit boundary for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--recorded-at",
        help="Optional recordedAt value for --build-receipt-from-trace.",
    )
    parser.add_argument(
        "--trace-session-id",
        help="Only include JSONL trace events from this sessionId.",
    )
    parser.add_argument(
        "--trace-language-id",
        help="Only include JSONL trace events from this languageId.",
    )
    parser.add_argument(
        "--trace-provider-id",
        help="Only include JSONL trace events from this providerId.",
    )


def handle_trace_receipt_args(repo_root: Path, args: Any) -> int | None:
    trace_arg = getattr(args, "build_receipt_from_trace", None)
    if not trace_arg:
        return None
    trace_path = _required_cli_path(repo_root, trace_arg, "--build-receipt-from-trace")
    if trace_path is None:
        return 2
    config = TraceReceiptConfig(
        scenario_id=args.scenario_id,
        language=args.language,
        project_name=args.project_name,
        project_source=args.project_source,
        intent=args.intent,
        edit_boundary=args.edit_boundary,
        recorded_at=args.recorded_at,
    )
    filters = TraceCommandFilter(
        session_id=args.trace_session_id,
        language_id=args.trace_language_id,
        provider_id=args.trace_provider_id,
    )
    output_arg = getattr(args, "output", None)
    if output_arg:
        return _write_trace_receipt(repo_root, trace_path, output_arg, args, config, filters)
    emit_json(build_receipt_from_trace_path(trace_path, config=config, filters=filters))
    return 0


def _write_trace_receipt(
    repo_root: Path,
    trace_path: Path,
    output_arg: str,
    args: Any,
    config: TraceReceiptConfig,
    filters: TraceCommandFilter,
) -> int:
    output_path = _required_cli_path(repo_root, output_arg, "--output")
    if output_path is None:
        return 2
    write_receipt_from_trace_path(
        trace_path,
        output_path,
        config=config,
        filters=filters,
    )
    result = validate_receipt_path(repo_root, output_path)
    if args.json:
        emit_json(receipt_report_json([result]))
    else:
        print_receipt_report(repo_root, [result])
    return 0 if result.status == "pass" else 1


def _required_cli_path(repo_root: Path, value: str, option: str) -> Path | None:
    path = resolve_path(repo_root, value)
    if path is None:
        emit(f"{option}: invalid path: {value}", file=sys.stderr)
        return None
    return path
