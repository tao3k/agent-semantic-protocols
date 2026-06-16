"""CLI helpers for adaptive graph-turbo validation reports."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_adaptive_validation import (
    build_large_library_adaptive_validation_report,
)
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_adaptive_validation_arguments(
    parser: argparse.ArgumentParser,
) -> None:
    parser.add_argument(
        "--large-library-adaptive-validation",
        metavar="ADAPTIVE_POLICY_JSON",
        help=(
            "Validate a graph-turbo adaptive policy against aggregated "
            "question-level live-agent analysis."
        ),
    )
    parser.add_argument(
        "--question-plan",
        metavar="QUESTION_PLAN_JSON",
        help=(
            "Question improvement plan or aggregate used by "
            "--large-library-adaptive-validation."
        ),
    )


def handle_large_library_adaptive_validation_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_adaptive_validation
    if source is None:
        return None
    if args.question_plan is None:
        raise SystemExit(
            "--question-plan is required with --large-library-adaptive-validation"
        )
    policy = _load_json_object(resolve_path(repo_root, source))
    question_plan = _load_json_object(resolve_path(repo_root, args.question_plan))
    report = build_large_library_adaptive_validation_report(policy, question_plan)
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), report)
    elif args.json:
        emit_json(report)
    else:
        _print_report(report)
    if args.fail_on_missing and report["status"] != "complete":
        return 1
    return 0


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _print_report(report: dict[str, object]) -> None:
    summary = report.get("summary")
    if not isinstance(summary, dict):
        emit("[large-library-adaptive-validation] invalid")
        return
    emit(
        "[large-library-adaptive-validation] "
        f"status={report.get('status')} "
        f"planned={summary.get('plannedRunCount')} "
        f"observed={summary.get('observedRunCount')} "
        f"missing={summary.get('missingRunCount')} "
        f"coverage={summary.get('coverageRatio')}"
    )
    readiness = report.get("promotionReadiness")
    if isinstance(readiness, dict):
        emit(
            "|promotion "
            f"status={readiness.get('status')} "
            f"blockingReasons={','.join(readiness.get('blockingReasons', []))}"
        )
