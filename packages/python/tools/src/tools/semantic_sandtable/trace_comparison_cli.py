"""CLI adapter for comparing failure-frontier trace pairs."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

from .failure_frontier_eval import (
    compare_failure_frontier_receipts,
    print_failure_frontier_comparison,
)
from .failure_frontier_scenario import failure_frontier_thresholds
from .output import emit_json
from .trace_receipt_events import TraceCommandFilter
from .trace_receipts import TraceReceiptConfig, build_receipt_from_trace_path
from .utils import resolve_path


def add_trace_comparison_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--compare-traces",
        nargs=2,
        metavar=("BASELINE_TRACE", "CANDIDATE_TRACE"),
        help=(
            "Build in-memory receipts from two trace roots or files, then compare "
            "them with the failure-frontier gate."
        ),
    )
    parser.add_argument(
        "--baseline-trace-session-id",
        help="Only include baseline JSONL trace events from this sessionId.",
    )
    parser.add_argument(
        "--candidate-trace-session-id",
        help="Only include candidate JSONL trace events from this sessionId.",
    )


def handle_trace_comparison_args(repo_root: Path, args: Any) -> int | None:
    trace_pair = getattr(args, "compare_traces", None)
    if not trace_pair:
        return None
    baseline_path, candidate_path = _trace_pair_paths(repo_root, trace_pair)
    comparison = compare_failure_frontier_receipts(
        _trace_receipt(baseline_path, args, "baseline"),
        _trace_receipt(candidate_path, args, "candidate"),
        expected_hot_blocks=args.expected_hot_block,
        thresholds=failure_frontier_thresholds(_threshold_args(args)),
    )
    if args.json:
        emit_json(comparison)
    else:
        print_failure_frontier_comparison(comparison)
    return 0 if comparison["status"] == "pass" else 1


def _trace_pair_paths(repo_root: Path, trace_pair: list[str]) -> tuple[Path, Path]:
    baseline, candidate = trace_pair
    return _trace_path(repo_root, baseline), _trace_path(repo_root, candidate)


def _trace_path(repo_root: Path, value: str) -> Path:
    return resolve_path(repo_root, value) or (repo_root / value).resolve()


def _trace_receipt(trace_path: Path, args: Any, side: str) -> dict[str, Any]:
    return build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id=f"{args.scenario_id}.{side}",
            language=args.language,
            project_name=args.project_name,
            project_source=args.project_source,
            intent=args.intent,
            edit_boundary=args.edit_boundary,
            recorded_at=args.recorded_at,
        ),
        filters=_trace_filter(args, side),
    )


def _trace_filter(args: Any, side: str) -> TraceCommandFilter:
    session_id = getattr(args, f"{side}_trace_session_id")
    return TraceCommandFilter(
        session_id=session_id,
        language_id=args.trace_language_id,
        provider_id=args.trace_provider_id,
    )


def _threshold_args(args: Any) -> dict[str, Any]:
    return {
        "minCommandReduction": args.min_command_reduction,
        "maxDirectSourceReadCode": args.max_direct_source_read_code,
        "maxDuplicateSelectors": args.max_duplicate_selectors,
        "maxSameFileWindowFanout": args.max_same_file_window_fanout,
    }
