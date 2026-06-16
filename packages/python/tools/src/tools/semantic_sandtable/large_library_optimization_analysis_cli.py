"""CLI helpers for large-library optimization analysis."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_optimization_analysis import (
    build_large_library_optimization_analysis,
)
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_optimization_analysis_arguments(
    parser: argparse.ArgumentParser,
) -> None:
    parser.add_argument(
        "--large-library-optimization-analysis",
        metavar="REPORT_CHAIN_JSON",
        help="Analyze large-library optimization batch results from a report chain.",
    )
    parser.add_argument(
        "--optimization-result",
        action="append",
        default=[],
        help="Variant result JSON file. May contain one object or an array.",
    )


def handle_large_library_optimization_analysis_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_optimization_analysis
    if source is None:
        return None
    report_chain = _load_json_object(resolve_path(repo_root, source))
    results = _load_result_files(repo_root, args.optimization_result)
    analysis = build_large_library_optimization_analysis(report_chain, results)
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), analysis)
    elif args.json:
        emit_json(analysis)
    else:
        _print_analysis(analysis)
    if args.fail_on_missing and analysis["summary"]["status"] != "analyzed":
        return 1
    return 0


def _load_result_files(repo_root: Path, paths: list[str]) -> list[dict[str, object]]:
    results: list[dict[str, object]] = []
    for path in paths:
        value = _load_json(resolve_path(repo_root, path))
        if isinstance(value, list):
            results.extend(item for item in value if isinstance(item, dict))
        elif isinstance(value, dict):
            results.append(value)
    return results


def _load_json_object(path: Path) -> dict[str, object]:
    value = _load_json(path)
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def _print_analysis(analysis: dict[str, object]) -> None:
    summary = analysis["summary"]
    if not isinstance(summary, dict):
        emit("[large-library-optimization-analysis] invalid")
        return
    emit(
        "[large-library-optimization-analysis] "
        f"status={summary.get('status')} "
        f"expected={summary.get('expectedVariantRunCount')} "
        f"observed={summary.get('observedVariantRunCount')} "
        f"missing={summary.get('missingVariantRunCount')} "
        f"findings={summary.get('findingCount')}"
    )
    collection = analysis.get("collectionManifest")
    if isinstance(collection, dict):
        missing = collection.get("missingVariantRuns")
        if isinstance(missing, list) and missing:
            first = missing[0]
            if isinstance(first, dict):
                emit(f"|missing variantRunId={first.get('variantRunId')}")
        needs_receipt = collection.get("runsNeedingVariantReceipt")
        if isinstance(needs_receipt, list) and needs_receipt:
            first = needs_receipt[0]
            if isinstance(first, dict):
                emit(
                    "|collect "
                    f"variantRunId={first.get('variantRunId')} "
                    f"status={first.get('collectionStatus')} "
                    f"source={first.get('metricSource')}"
                )
    recommendations = analysis.get("variantRecommendations")
    if isinstance(recommendations, dict):
        winner = recommendations.get("overallWinner")
        if isinstance(winner, dict):
            emit(
                "|recommendation "
                f"overallWinner={winner.get('ablationVariant')} "
                f"averageElapsedMs={winner.get('averageElapsedMs')} "
                f"averageAnswerQuality={winner.get('averageAnswerQuality')}"
            )
    for finding in analysis.get("findings", []):
        if isinstance(finding, dict):
            emit(
                "|finding "
                f"kind={finding.get('kind')} "
                f"severity={finding.get('severity')} "
                f"message={finding.get('message')}"
            )
