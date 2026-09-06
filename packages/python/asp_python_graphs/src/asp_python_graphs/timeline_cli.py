# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""CLI for cached ASP artifact timeline and microburst audits."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .artifact_events import artifact_events_from_packet
from .artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_events_timeline,
    evaluate_artifact_timeline,
)
from .artifact_timeline_parameters import parse_since, parse_timeline_args
from .artifact_timeline_text import write_timeline_text_report


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    parameters = TimelineParameters(
        subagent_start_gap_seconds=args.subagent_start_gap_seconds,
        subagent_soft_max_seconds=args.subagent_soft_max_seconds,
        subagent_hard_max_seconds=args.subagent_hard_max_seconds,
        session_gap_seconds=args.session_gap_seconds,
        examples=args.examples,
        since_timestamp=_parse_since(args.since),
        recent_sessions=args.recent_sessions,
    )
    if args.events_json is None:
        report = evaluate_artifact_timeline(args.artifact_dir, parameters=parameters)
    else:
        if str(args.events_json) == "-":
            packet = json.load(sys.stdin)
        else:
            packet = json.loads(args.events_json.read_text(encoding="utf-8"))
        events = artifact_events_from_packet(packet)
        artifact_dir = Path(str(packet.get("artifactDir") or args.artifact_dir))
        source = packet.get("source") if isinstance(packet, dict) else None
        event_source = (
            source.get("kind")
            if isinstance(source, dict) and isinstance(source.get("kind"), str)
            else "events-json"
        )
        report = evaluate_artifact_events_timeline(
            events,
            artifact_dir=artifact_dir,
            parameters=parameters,
            event_source=event_source,
        )
    if args.format == "json":
        sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    else:
        write_timeline_text_report(report)
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    return parse_timeline_args(argv)


def _parse_since(value: str | None) -> float | None:
    return parse_since(value)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
