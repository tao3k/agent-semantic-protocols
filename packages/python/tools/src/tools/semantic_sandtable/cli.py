"""Command-line interface for semantic sandtable scenarios."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .constants import COVERAGE_POLICY_PATH
from .coverage import coverage_report
from .failure_frontier_eval import (
    FailureFrontierThresholds,
    compare_failure_frontier_receipt_paths,
    print_failure_frontier_comparison,
)
from .models import ScenarioLoadError, has_warnings
from .output import emit, emit_json
from .receipts import validate_receipt_path
from .reports import (
    coverage_report_json,
    print_coverage_report,
    print_receipt_report,
    print_text_report,
    receipt_report_json,
    report_json,
)
from .scenario_io import discover_scenarios, load_scenario
from .scenario_runner import run_scenario
from .trace_comparison_cli import (
    add_trace_comparison_arguments,
    handle_trace_comparison_args,
)
from .trace_receipt_cli import add_trace_receipt_arguments, handle_trace_receipt_args
from .trace_record_cli import add_trace_record_arguments, handle_trace_record_args
from .trace_sessions_cli import add_trace_session_arguments, handle_trace_session_args
from .utils import resolve_path


def semantic_sandtable_main(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    repo_root = Path(args.repo_root).expanduser().resolve()

    direct_result = _handle_direct_commands(repo_root, args)
    if direct_result is not None:
        return direct_result

    scenario_paths = discover_scenarios(repo_root, args.scenarios)
    if args.list:
        return _list_scenarios(repo_root, scenario_paths)
    if args.coverage:
        return _coverage(repo_root, scenario_paths, args)
    return _run_scenarios(repo_root, scenario_paths, args)


def _handle_direct_commands(repo_root: Path, args: argparse.Namespace) -> int | None:
    trace_result = handle_trace_receipt_args(repo_root, args)
    if trace_result is not None:
        return trace_result
    trace_record_result = handle_trace_record_args(repo_root, args)
    if trace_record_result is not None:
        return trace_record_result
    trace_session_result = handle_trace_session_args(repo_root, args)
    if trace_session_result is not None:
        return trace_session_result
    trace_compare_result = handle_trace_comparison_args(repo_root, args)
    if trace_compare_result is not None:
        return trace_compare_result
    if args.compare_receipts:
        return _compare_receipts(repo_root, args)
    if args.receipt:
        return _validate_receipts(repo_root, args)
    return None


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="semantic-sandtable",
        description="Run semantic language harness sandtable scenarios.",
    )
    _add_general_arguments(parser)
    _add_receipt_arguments(parser)
    add_trace_receipt_arguments(parser)
    add_trace_record_arguments(parser)
    add_trace_session_arguments(parser)
    add_trace_comparison_arguments(parser)
    _add_failure_frontier_arguments(parser)
    _add_coverage_arguments(parser)
    return parser


def _add_general_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "scenarios",
        nargs="*",
        help="Scenario JSON files. Defaults to sandtables/**/*.json.",
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Protocol repository root. Defaults to current directory.",
    )
    parser.add_argument("--json", action="store_true", help="Emit JSON report.")
    parser.add_argument(
        "--list",
        action="store_true",
        help="List discovered scenarios without running them.",
    )
    parser.add_argument(
        "--coverage",
        action="store_true",
        help="Audit declared scenario coverage without executing commands.",
    )


def _add_receipt_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        help="Validate a real-trigger receipt JSON file and print a compact summary.",
    )


def _add_failure_frontier_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--compare-receipts",
        nargs=2,
        metavar=("BASELINE", "CANDIDATE"),
        help=(
            "Compare baseline and candidate real-trigger receipts for "
            "failure-frontier command reduction."
        ),
    )
    parser.add_argument(
        "--expected-hot-block",
        action="append",
        default=[],
        help=(
            "Expected hot block selector for --compare-receipts or --compare-traces. "
            "May be repeated."
        ),
    )
    parser.add_argument(
        "--min-command-reduction",
        type=float,
        default=0.5,
        help="Minimum command reduction ratio for failure-frontier comparisons.",
    )
    parser.add_argument(
        "--max-direct-source-read-code",
        type=int,
        default=4,
        help="Maximum candidate direct-source-read --code commands.",
    )
    parser.add_argument(
        "--max-duplicate-selectors",
        type=int,
        default=0,
        help="Maximum duplicate candidate direct-source-read selectors.",
    )
    parser.add_argument(
        "--max-same-file-window-fanout",
        type=int,
        default=0,
        help="Maximum same-file direct-source-read window fanout in candidate.",
    )


def _add_coverage_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--coverage-policy",
        default=str(COVERAGE_POLICY_PATH),
        help="Coverage policy JSON for per-language audit expectations.",
    )
    parser.add_argument(
        "--fail-on-missing",
        action="store_true",
        help="Return non-zero if coverage audit reports missing surfaces.",
    )
    parser.add_argument(
        "--fail-on-warn",
        action="store_true",
        help="Return non-zero if any warning budget is exceeded.",
    )


def _compare_receipts(repo_root: Path, args: argparse.Namespace) -> int:
    baseline, candidate = args.compare_receipts
    comparison = compare_failure_frontier_receipt_paths(
        repo_root,
        Path(baseline).expanduser(),
        Path(candidate).expanduser(),
        expected_hot_blocks=args.expected_hot_block,
        thresholds=FailureFrontierThresholds(
            min_command_reduction=args.min_command_reduction,
            max_direct_source_read_code=args.max_direct_source_read_code,
            max_duplicate_selectors=args.max_duplicate_selectors,
            max_same_file_window_fanout=args.max_same_file_window_fanout,
        ),
    )
    if args.json:
        emit_json(comparison)
    else:
        print_failure_frontier_comparison(comparison)
    return 0 if comparison["status"] == "pass" else 1


def _validate_receipts(repo_root: Path, args: argparse.Namespace) -> int:
    receipt_results = [
        validate_receipt_path(repo_root, Path(receipt).expanduser())
        for receipt in args.receipt
    ]
    if args.json:
        emit_json(receipt_report_json(receipt_results))
    else:
        print_receipt_report(repo_root, receipt_results)
    return 1 if any(result.status == "fail" for result in receipt_results) else 0


def _list_scenarios(repo_root: Path, scenario_paths: list[Path]) -> int:
    for path in scenario_paths:
        try:
            scenario = load_scenario(path, repo_root)
        except ScenarioLoadError as error:
            emit(f"[sandtable-error] {error}", file=sys.stderr)
            return 1
        emit(
            f"{scenario.get('id', path.stem)}\t"
            f"{scenario.get('language', 'unknown')}\t"
            f"{path.relative_to(repo_root)}"
        )
    return 0


def _coverage(
    repo_root: Path,
    scenario_paths: list[Path],
    args: argparse.Namespace,
) -> int:
    policy_path = resolve_path(repo_root, args.coverage_policy)
    coverage = coverage_report(repo_root, scenario_paths, policy_path=policy_path)
    if args.json:
        emit_json(coverage_report_json(coverage))
    else:
        print_coverage_report(coverage)
    missing = bool(
        coverage.missing
        or coverage.language_missing
        or coverage.large_library_missing
    )
    if coverage.errors or (args.fail_on_missing and missing):
        return 1
    return 0


def _run_scenarios(
    repo_root: Path,
    scenario_paths: list[Path],
    args: argparse.Namespace,
) -> int:
    results = [run_scenario(repo_root, path) for path in scenario_paths]
    if args.json:
        emit_json(report_json(results))
    else:
        print_text_report(repo_root, results)

    failed = any(result.status == "fail" for result in results)
    warned = any(has_warnings(result) for result in results)
    if failed or (args.fail_on_warn and warned):
        return 1
    return 0
