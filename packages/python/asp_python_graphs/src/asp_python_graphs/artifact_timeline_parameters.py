# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Timeline parameter model and parameter-row projection."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Sequence

from .artifact_events import ArtifactEvent, scan_artifact_events


@dataclass(frozen=True)
class TimelineParameters:
    subagent_start_gap_seconds: int = 10
    subagent_soft_max_seconds: int = 30
    subagent_hard_max_seconds: int = 60
    session_gap_seconds: int = 600
    examples: int = 5
    since_timestamp: float | None = None
    recent_sessions: int | None = None


def parse_timeline_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse timeline controls shared by offline evidence and the gRPC service."""

    parser = argparse.ArgumentParser(description="Timeline parameter controls")
    parser.add_argument(
        "artifact_dir",
        nargs="?",
        type=Path,
        default=Path(".cache/agent-semantic-protocol/artifacts"),
    )
    parser.add_argument("--events-json", type=Path)
    parser.add_argument("--subagent-start-gap-seconds", type=int, default=10)
    parser.add_argument("--subagent-soft-max-seconds", type=int, default=30)
    parser.add_argument("--subagent-hard-max-seconds", type=int, default=60)
    parser.add_argument("--session-gap-seconds", type=int, default=600)
    parser.add_argument("--examples", type=int, default=5)
    parser.add_argument("--since")
    parser.add_argument("--recent-sessions", type=int)
    parser.add_argument("--format", choices=["text", "json"], default="text")
    return parser.parse_args(argv)


def parse_since(value: str | None) -> float | None:
    if value is None:
        return None
    stripped = value.strip()
    if not stripped:
        return None
    try:
        return float(stripped)
    except ValueError:
        normalized = stripped[:-1] + "+00:00" if stripped.endswith("Z") else stripped
        return datetime.fromisoformat(normalized).timestamp()


def filtered_events(root: Path, params: TimelineParameters) -> tuple[ArtifactEvent, ...]:
    events = scan_artifact_events(root)
    if params.since_timestamp is not None:
        return tuple(
            event for event in events if event.timestamp >= params.since_timestamp
        )
    return events


def parameter_row(params: TimelineParameters) -> dict[str, object]:
    return {
        "subagentStartGapSeconds": params.subagent_start_gap_seconds,
        "subagentSoftMaxSeconds": params.subagent_soft_max_seconds,
        "subagentHardMaxSeconds": params.subagent_hard_max_seconds,
        "sessionGapSeconds": params.session_gap_seconds,
        "since": (
            timestamp(params.since_timestamp)
            if params.since_timestamp is not None
            else None
        ),
        "recentSessions": params.recent_sessions,
    }


def timestamp(value: float) -> str:
    return datetime.fromtimestamp(value).isoformat(timespec="seconds")
