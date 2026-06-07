"""Render RFC 011 syntax real-project evidence records."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path


COMMANDS = (
    "search-prime",
    "syntax-frontier",
    "exact-selector-code",
    "hook-recovery",
)

OUTPUTS = (
    "frontier-no-code",
    "pure-code-stdout",
    "registry-descriptor",
    "query-corpus",
)

METRICS_LINE_ONE = (
    "commandCount",
    "providerProcessCount",
    "packetBytes",
    "coldElapsedMs",
    "warmElapsedMs",
)

METRICS_LINE_TWO = (
    "syntaxQueryCount",
    "exactCodeCount",
    "manualRangeScanCount",
    "repeatedTriggerReduction",
)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    error = _validate_cache_claim(args)
    if error is not None:
        sys.stderr.write(f"{error}\n")
        return 2

    record = _render_record(args)
    if args.output is not None:
        args.output.write_text(record + "\n", encoding="utf-8")
    else:
        sys.stdout.write(f"{record}\n")
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="python -m tools syntax real-evidence",
        description=(
            "Render the RFC 011 [syntax-real-evidence] review record after a "
            "real Rust or TypeScript syntax-query flow has been measured."
        )
    )
    parser.add_argument("--language", required=True, choices=("rust", "typescript"))
    parser.add_argument("--provider", required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument("--command-count", type=_non_negative_int, required=True)
    parser.add_argument("--provider-process-count", type=_non_negative_int, required=True)
    parser.add_argument("--packet-bytes", type=_non_negative_int, required=True)
    parser.add_argument("--cold-elapsed-ms", type=_non_negative_int, required=True)
    parser.add_argument("--warm-elapsed-ms", type=_non_negative_int, required=True)
    parser.add_argument("--syntax-query-count", type=_non_negative_int, required=True)
    parser.add_argument("--exact-code-count", type=_non_negative_int, required=True)
    parser.add_argument("--manual-range-scan-count", type=_non_negative_int, required=True)
    parser.add_argument("--repeated-trigger-reduction", type=_non_negative_int, required=True)
    parser.add_argument(
        "--cache-claim",
        choices=("none", "warm-provider", "hit"),
        default="none",
        help="Use hit only when a receipt or normalized-row replay proves it.",
    )
    parser.add_argument(
        "--cache-receipt",
        type=Path,
        help="Receipt path required when --cache-claim hit is used.",
    )
    parser.add_argument(
        "--normalized-row-replay",
        action="store_true",
        help="Mark cache hit as proven by validated normalized-row replay.",
    )
    parser.add_argument("--output", type=Path, help="Optional file to write.")
    return parser.parse_args(argv)


def _non_negative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be non-negative")
    return parsed


def _validate_cache_claim(args: argparse.Namespace) -> str | None:
    if args.cache_claim != "hit":
        return None
    if args.cache_receipt is None and not args.normalized_row_replay:
        return (
            "--cache-claim hit requires --cache-receipt or "
            "--normalized-row-replay"
        )
    return None


def _render_record(args: argparse.Namespace) -> str:
    lines = [
        (
            f"[syntax-real-evidence] language={args.language} "
            f"provider={args.provider} project={args.project}"
        ),
        f"commands={','.join(COMMANDS)}",
        "metrics=" + _metric_values(args, METRICS_LINE_ONE),
        "metrics=" + _metric_values(args, METRICS_LINE_TWO),
        f"outputs={','.join(OUTPUTS)}",
        f"cacheClaim={args.cache_claim}",
    ]
    if args.cache_receipt is not None:
        lines.append(f"cacheReceipt={args.cache_receipt}")
    if args.normalized_row_replay:
        lines.append("cacheReplayEvidence=normalized-row-replay")
    return "\n".join(lines)


def _metric_values(args: argparse.Namespace, metric_names: tuple[str, ...]) -> str:
    values = []
    for name in metric_names:
        values.append(f"{name}={getattr(args, _argument_name(name))}")
    return ",".join(values)


def _argument_name(metric_name: str) -> str:
    chars = []
    for char in metric_name:
        if char.isupper():
            chars.append("_")
            chars.append(char.lower())
        else:
            chars.append(char)
    return "".join(chars)


if __name__ == "__main__":
    raise SystemExit(main())
