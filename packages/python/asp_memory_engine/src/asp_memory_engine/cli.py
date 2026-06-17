"""CLI for the Python Xiuxian memory engine adaptation."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence

from .graph_turbo_memory import read_memory_projection


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    if args.command == "graph-turbo-read-memory":
        payload = json.load(sys.stdin)
        projection = read_memory_projection(
            payload.get("candidateSelectors", []),
            payload.get("seenSelectors", []),
            max_gap_lines=args.max_gap_lines,
        )
        sys.stdout.write(
            json.dumps(
                {
                    "seenSelectors": list(projection.seen_selectors),
                    "suppressedSelectors": list(projection.suppressed_selectors),
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 0
    raise SystemExit(f"unsupported command: {args.command}")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    read_memory = subparsers.add_parser("graph-turbo-read-memory")
    read_memory.add_argument("--max-gap-lines", type=int, default=8)
    return parser.parse_args(argv)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
