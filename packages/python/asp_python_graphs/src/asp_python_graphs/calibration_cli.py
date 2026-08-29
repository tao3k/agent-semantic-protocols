"""Build graph-turbo profile calibration packets from feedback facts."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .calibration import calibration_to_json, profile_calibration_from_feedback
from .profiles import DEFAULT_PROFILES


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    paths = list(args.paths)
    if len(paths) < 2:
        raise SystemExit(
            "calibrate requires at least one feedback packet and a request"
        )
    request_path = paths[-1]
    feedback_paths = paths[:-1]
    request = _load_json(request_path)
    feedback_packets = [_load_feedback_packet(path) for path in feedback_paths]
    sys.stdout.write(
        calibration_to_json(
            profile_calibration_from_feedback(
                feedback_packets,
                request,
                profile=args.profile,
            )
        )
    )
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="+",
        help="Feedback packet path(s) followed by the graph-turbo request packet.",
    )
    parser.add_argument(
        "--profile",
        default="owner-query",
        choices=sorted(DEFAULT_PROFILES),
        help="Profile whose scoring policy should receive calibration deltas.",
    )
    return parser.parse_args(argv)


def _load_feedback_packet(path: str) -> Mapping[str, object]:
    packet = _load_json(path)
    if (
        packet.get("schemaId")
        != "agent.semantic-protocols.semantic-graph-turbo-feedback"
    ):
        raise SystemExit(f"unsupported graph turbo feedback packet: {path}")
    return packet


def _load_json(path: str) -> Mapping[str, object]:
    if path == "-":
        payload = json.load(sys.stdin)
    else:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, Mapping):
        raise SystemExit("graph turbo calibration input must be a JSON object")
    return payload


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
