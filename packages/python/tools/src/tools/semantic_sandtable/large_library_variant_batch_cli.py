"""CLI for building large-library optimization variant-result batches."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_variant_batch import build_large_library_variant_batch
from .large_library_variant_result import parse_metric_values
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_variant_batch_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--large-library-variant-batch",
        metavar="REPORT_CHAIN_JSON",
        help=(
            "Build one large-library optimization variant-result packet for "
            "each variant run in a report chain."
        ),
    )
    parser.add_argument(
        "--source-sandtable-receipt",
        help="Sandtable JSON report used to derive per-scenario variant metrics.",
    )
    parser.add_argument(
        "--variant-sandtable-receipt",
        action="append",
        default=[],
        metavar="VARIANT_OR_RUN_ID=RECEIPT_JSON",
        help=(
            "Variant-specific sandtable receipt. The key may be an ablation "
            "variant name or a full variantRunId."
        ),
    )


def handle_large_library_variant_batch_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_variant_batch
    if source is None:
        return None
    report_chain = _load_json_object(resolve_path(repo_root, source))
    sandtable_receipt = _load_optional_json_object(
        repo_root, getattr(args, "source_sandtable_receipt", None)
    )
    variant_receipts, variant_paths = _load_variant_receipts(
        repo_root,
        args.variant_sandtable_receipt,
    )
    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=parse_metric_values(args.receipt_metric),
        answer_metrics=parse_metric_values(args.answer_metric),
        source_receipt_path=args.source_receipt_path,
        sandtable_receipt=sandtable_receipt,
        variant_sandtable_receipts=variant_receipts,
        variant_source_receipt_paths=variant_paths,
    )
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), packets)
    elif args.json:
        emit_json(packets)
    else:
        emit(f"[large-library-variant-batch] variants={len(packets)}")
    return 0


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _load_optional_json_object(
    repo_root: Path,
    value: str | None,
) -> dict[str, object] | None:
    if value is None:
        return None
    return _load_json_object(resolve_path(repo_root, value))


def _load_variant_receipts(
    repo_root: Path,
    values: list[str],
) -> tuple[dict[str, dict[str, object]], dict[str, str]]:
    receipts: dict[str, dict[str, object]] = {}
    paths: dict[str, str] = {}
    for value in values:
        key, path = _split_variant_receipt(value)
        resolved = resolve_path(repo_root, path)
        receipts[key] = _load_json_object(resolved)
        paths[key] = str(resolved)
    return receipts, paths


def _split_variant_receipt(value: str) -> tuple[str, str]:
    if "=" not in value:
        raise SystemExit(
            "--variant-sandtable-receipt must use VARIANT_OR_RUN_ID=RECEIPT_JSON"
        )
    key, path = value.split("=", 1)
    if not key or not path:
        raise SystemExit(
            "--variant-sandtable-receipt must use VARIANT_OR_RUN_ID=RECEIPT_JSON"
        )
    return key, path
