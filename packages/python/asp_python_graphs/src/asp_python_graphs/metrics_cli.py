"""Render graph turbo real-trigger metric records."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Sequence

from .constants import ALGORITHM_ID
from .real_trigger_metrics import (
    build_real_trigger_metrics,
    render_real_trigger_metrics,
)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        record = build_real_trigger_metrics(
            scenario=args.scenario,
            source=args.source,
            measured_at=args.measured_at,
            profile=args.profile,
            algorithm=args.algorithm,
            commands=args.command,
            command_count=args.command_count,
            packet_bytes=args.packet_bytes,
            result_bytes=args.result_bytes,
            latency_ms=args.latency_ms,
            repeated_trigger_patterns=args.repeated_trigger_patterns,
            missing_facts=args.missing_facts,
            confusing_next_actions=args.confusing_next_actions,
        )
    except ValueError as error:
        sys.stderr.write(f"graph-turbo metrics: {error}\n")
        return 2

    if args.format == "json":
        sys.stdout.write(json.dumps(record.to_packet(), sort_keys=True) + "\n")
    else:
        sys.stdout.write(render_real_trigger_metrics(record) + "\n")
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--source", default="live-cli")
    parser.add_argument("--measured-at", required=True)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--algorithm", default=ALGORITHM_ID)
    parser.add_argument("--command", action="append", default=[])
    parser.add_argument("--command-count", type=_positive_int, required=True)
    parser.add_argument("--packet-bytes", type=_non_negative_int, required=True)
    parser.add_argument("--result-bytes", type=_non_negative_int, required=True)
    parser.add_argument("--latency-ms", type=_non_negative_int, required=True)
    parser.add_argument(
        "--repeated-trigger-patterns",
        type=_non_negative_int,
        required=True,
    )
    parser.add_argument("--missing-facts", type=_non_negative_int, required=True)
    parser.add_argument(
        "--confusing-next-actions",
        type=_non_negative_int,
        required=True,
    )
    parser.add_argument("--format", choices=["text", "json"], default="text")
    return parser.parse_args(argv)


def _positive_int(value: str) -> int:
    parsed = _non_negative_int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def _non_negative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be non-negative")
    return parsed


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
