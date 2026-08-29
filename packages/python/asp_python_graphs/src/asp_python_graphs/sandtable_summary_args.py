"""Argument parsing for graph turbo sandtable summary CLI."""

from __future__ import annotations

import argparse
from collections.abc import Sequence


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = _build_parser()
    return parser.parse_args(argv)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Summarize graph turbo benchmark and receipt packets."
    )
    _add_benchmark_source_args(parser)
    _add_benchmark_runtime_args(parser)
    _add_report_args(parser)
    _add_gate_args(parser)
    return parser


def _add_benchmark_source_args(parser: argparse.ArgumentParser) -> None:
    benchmark_source = parser.add_mutually_exclusive_group(required=True)
    benchmark_source.add_argument("--benchmark")
    benchmark_source.add_argument("--benchmark-packet")


def _add_benchmark_runtime_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--benchmark-runs", type=_positive_int, default=30)
    parser.add_argument("--benchmark-warmup-runs", type=_non_negative_int, default=3)
    parser.add_argument(
        "--benchmark-cache-mode",
        choices=["packet", "enabled", "disabled"],
        default="packet",
    )
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)


def _add_report_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--receipt")
    parser.add_argument("--receipt-fixture-id")
    parser.add_argument("--benchmark-report")
    parser.add_argument("--report-scenario")
    parser.add_argument("--large-library-report-chain")
    parser.add_argument("--scenario")
    parser.add_argument("--format", choices=["json", "text"], default="json")


def _add_gate_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--fail-on-gate", action="store_true")
    parser.add_argument("--max-ppr-iterations", type=_positive_int, default=100)
    parser.add_argument("--ppr-mass-tolerance", type=float, default=0.000001)
    parser.add_argument("--min-frontier-follow-rate", type=float, default=0.0)
    parser.add_argument(
        "--max-raw-read-fallback-count", type=_non_negative_int, default=0
    )
    parser.add_argument(
        "--max-duplicate-selector-count", type=_non_negative_int, default=0
    )
    parser.add_argument(
        "--max-same-owner-scan-count", type=_non_negative_int, default=0
    )
    parser.add_argument(
        "--max-commands-to-first-useful-locator", type=_non_negative_int
    )
    parser.add_argument("--max-p95-ms", type=float)


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def _non_negative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must be non-negative")
    return parsed
