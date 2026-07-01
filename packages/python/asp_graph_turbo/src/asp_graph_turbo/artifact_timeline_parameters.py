"""Timeline parameter model and parameter-row projection."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

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
