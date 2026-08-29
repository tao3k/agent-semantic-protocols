"""Build graph-turbo feedback packets from sandtable reports."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .feedback import feedback_packet_from_sandtable


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    report = _load_json(args.report)
    packet = feedback_packet_from_sandtable(
        report,
        source_path=None if args.report == "-" else args.report,
    )
    sys.stdout.write(json.dumps(packet, sort_keys=True) + "\n")
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "report",
        nargs="?",
        default="-",
        help="Sandtable JSON report path, or '-' for stdin.",
    )
    return parser.parse_args(argv)


def _load_json(path: str) -> Mapping[str, object]:
    if path == "-":
        payload = json.load(sys.stdin)
    else:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, Mapping):
        raise SystemExit("sandtable feedback input must be a JSON object")
    return payload


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
