"""Project artifact timeline events into sessions and microbursts."""

from __future__ import annotations

from collections import Counter
from datetime import datetime

from .artifact_events import ArtifactEvent
from .artifact_timeline_keys import event_example, fanout_key, key_row
from .artifact_timeline_parameters import TimelineParameters, timestamp


def selected_sessions(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> tuple[tuple[tuple[ArtifactEvent, ...], ...], tuple[ArtifactEvent, ...]]:
    sessions = _sessions_from_events(events, params.session_gap_seconds)
    if params.recent_sessions is not None:
        sessions = sessions[-params.recent_sessions :]
        events = tuple(event for session in sessions for event in session)
    return sessions, events


def session_row(
    session: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> dict[str, object]:
    bursts = microburst_rows(session, params)
    reasoning_gaps = _reasoning_gaps(bursts, params.subagent_start_gap_seconds)
    return {
        "start": timestamp(session[0].timestamp),
        "end": timestamp(session[-1].timestamp),
        "durationSeconds": round(session[-1].timestamp - session[0].timestamp, 3),
        "events": len(session),
        "actions": sum(1 for event in session if event.action),
        "microbursts": len(bursts),
        "fanoutBursts": sum(1 for burst in bursts if burst["fanoutWidth"] >= 2),
        "softOverrunMicrobursts": soft_overrun_count(bursts, params),
        "inferredSubagentStarts": sum(
            int(burst["fanoutWidth"])
            for burst in bursts
            if int(burst["fanoutWidth"]) >= 2
        ),
        "repeatSearches": _repeat_searches(session),
        "maxFanoutWidth": max(
            (int(burst["fanoutWidth"]) for burst in bursts), default=0
        ),
        "llmReasoningGaps": len(reasoning_gaps),
        "maxReasoningGapSeconds": max(reasoning_gaps, default=0.0),
        "kindCounts": dict(sorted(Counter(event.kind for event in session).items())),
    }


def microburst_rows(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> list[dict[str, object]]:
    bursts = _microbursts(
        tuple(event for event in events if event.action),
        gap_seconds=params.subagent_start_gap_seconds,
        hard_max_seconds=params.subagent_hard_max_seconds,
    )
    return [_burst_row(burst) for burst in bursts]


def top_microbursts(
    bursts: list[dict[str, object]], examples: int
) -> list[dict[str, object]]:
    return sorted(
        bursts,
        key=lambda burst: (
            -int(burst["fanoutWidth"]),
            -int(burst["events"]),
            -float(burst["spanSeconds"]),
        ),
    )[:examples]


def soft_overrun_count(
    bursts: list[dict[str, object]], params: TimelineParameters
) -> int:
    return sum(
        1
        for burst in bursts
        if float(burst["spanSeconds"]) > params.subagent_soft_max_seconds
    )


def action_method_counts(events: tuple[ArtifactEvent, ...]) -> dict[str, int]:
    counts = Counter(
        f"{event.language}:{event.method}" for event in events if event.action
    )
    return dict(sorted(counts.items()))


def _sessions_from_events(
    events: tuple[ArtifactEvent, ...], session_gap_seconds: int
) -> tuple[tuple[ArtifactEvent, ...], ...]:
    sessions: list[list[ArtifactEvent]] = []
    current: list[ArtifactEvent] = []
    previous = None
    for event in events:
        if previous is not None and event.timestamp - previous > session_gap_seconds:
            sessions.append(current)
            current = []
        current.append(event)
        previous = event.timestamp
    if current:
        sessions.append(current)
    return tuple(tuple(session) for session in sessions)


def _microbursts(
    events: tuple[ArtifactEvent, ...], *, gap_seconds: int, hard_max_seconds: int
) -> tuple[tuple[ArtifactEvent, ...], ...]:
    bursts: list[list[ArtifactEvent]] = []
    current: list[ArtifactEvent] = []
    for event in events:
        if current and _split_burst(current, event, gap_seconds, hard_max_seconds):
            bursts.append(current)
            current = []
        current.append(event)
    if current:
        bursts.append(current)
    return tuple(tuple(burst) for burst in bursts)


def _split_burst(
    current: list[ArtifactEvent],
    event: ArtifactEvent,
    gap_seconds: int,
    hard_max_seconds: int,
) -> bool:
    return (
        event.timestamp - current[-1].timestamp > gap_seconds
        or event.timestamp - current[0].timestamp > hard_max_seconds
    )


def _burst_row(burst: tuple[ArtifactEvent, ...]) -> dict[str, object]:
    keys = {fanout_key(event) for event in burst}
    return {
        "start": timestamp(burst[0].timestamp),
        "end": timestamp(burst[-1].timestamp),
        "spanSeconds": round(burst[-1].timestamp - burst[0].timestamp, 3),
        "events": len(burst),
        "fanoutWidth": len(keys),
        "commands": sum(1 for event in burst if event.kind == "command"),
        "searches": sum(1 for event in burst if event.kind == "search"),
        "queries": sum(1 for event in burst if event.kind == "query"),
        "methods": dict(sorted(Counter(event.method for event in burst).items())),
        "fanoutKeys": [key_row(key) for key in sorted(keys)],
        "examples": [event_example(event) for event in burst[:5]],
    }


def _repeat_searches(events: tuple[ArtifactEvent, ...]) -> int:
    counts = Counter(
        fanout_key(event)
        for event in events
        if event.action and event.method.startswith("search/")
    )
    return sum(count - 1 for count in counts.values() if count > 1)


def _reasoning_gaps(bursts: list[dict[str, object]], gap_seconds: int) -> list[float]:
    gaps: list[float] = []
    for previous, current in zip(bursts, bursts[1:]):
        gap = _parse_time(str(current["start"])) - _parse_time(str(previous["end"]))
        if gap > gap_seconds:
            gaps.append(round(gap, 3))
    return gaps


def _parse_time(value: str) -> float:
    return datetime.fromisoformat(value).timestamp()
