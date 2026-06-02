"""Command-line interface for semantic sandtable scenarios."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .constants import COVERAGE_POLICY_PATH
from .coverage import coverage_report
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
from .utils import resolve_path


def semantic_sandtable_main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="semantic-sandtable",
        description="Run semantic language harness sandtable scenarios.",
    )
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
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        help="Validate a real-trigger receipt JSON file and print a compact summary.",
    )
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
    args = parser.parse_args(argv)

    repo_root = Path(args.repo_root).expanduser().resolve()
    if args.receipt:
        receipt_results = [
            validate_receipt_path(repo_root, Path(receipt).expanduser())
            for receipt in args.receipt
        ]
        if args.json:
            emit_json(receipt_report_json(receipt_results))
        else:
            print_receipt_report(repo_root, receipt_results)
        return 1 if any(result.status == "fail" for result in receipt_results) else 0

    scenario_paths = discover_scenarios(repo_root, args.scenarios)
    if args.list:
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

    if args.coverage:
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
