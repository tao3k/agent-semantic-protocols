"""CLI helpers for large-library optimization variant results."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_variant_result import (
    build_large_library_variant_result,
    parse_metric_values,
)
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_variant_result_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--large-library-variant-result",
        metavar="REPORT_CHAIN_JSON",
        help="Build one large-library optimization variant result packet.",
    )
    parser.add_argument("--variant-run-id")
    parser.add_argument("--source-receipt-path")
    parser.add_argument("--receipt-metric", action="append", default=[])
    parser.add_argument("--answer-metric", action="append", default=[])


def handle_large_library_variant_result_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_variant_result
    if source is None:
        return None
    if not args.variant_run_id:
        raise SystemExit("--variant-run-id is required")
    try:
        report_chain = _load_json_object(resolve_path(repo_root, source))
        packet = build_large_library_variant_result(
            report_chain,
            variant_run_id=args.variant_run_id,
            receipt_metrics=parse_metric_values(args.receipt_metric),
            answer_metrics=parse_metric_values(args.answer_metric),
            source_receipt_path=args.source_receipt_path,
        )
    except ValueError as error:
        raise SystemExit(str(error)) from error
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), packet)
    elif args.json:
        emit_json(packet)
    else:
        emit(
            "[large-library-variant-result] "
            f"variantRunId={packet['variantRunId']} "
            f"language={packet['language']} "
            f"depth={packet['depthBucket']} "
            f"variant={packet['ablationVariant']}"
        )
    return 0


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)
