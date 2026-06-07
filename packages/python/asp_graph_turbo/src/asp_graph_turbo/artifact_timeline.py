"""Timeline and microburst evaluation for cached ASP artifacts."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from .artifact_action_summary import action_summary
from .artifact_efficiency import efficiency_estimate
from .artifact_events import ArtifactEvent, scan_artifact_events
from .artifact_fanout_planning import fanout_planning_candidates
from .artifact_fzf_promotion import fzf_promotion_candidates
from .artifact_owner_collapse import owner_collapse_candidates
from .artifact_prime_suppression import prime_suppression_candidates
from .artifact_read_loop import read_loop_risk_summary
from .artifact_timeline_targets import optimization_targets


@dataclass(frozen=True)
class TimelineParameters:
    subagent_start_gap_seconds: int = 10
    subagent_soft_max_seconds: int = 30
    subagent_hard_max_seconds: int = 60
    session_gap_seconds: int = 600
    examples: int = 5
    since_timestamp: float | None = None
    recent_sessions: int | None = None


@dataclass(frozen=True)
class TimelineContext:
    events: tuple[ArtifactEvent, ...]
    sessions: tuple[tuple[ArtifactEvent, ...], ...]
    session_rows: list[dict[str, object]]
    burst_rows: list[dict[str, object]]
    repeat_groups: list[dict[str, object]]
    fanout_hotspots: list[dict[str, object]]
    prime_suppression: dict[str, object]
    fzf_promotion: dict[str, object]
    owner_collapse: dict[str, object]
    fanout_planning: dict[str, object]
    read_loop_risk: dict[str, object]


def evaluate_artifact_timeline(
    root: Path,
    *,
    parameters: TimelineParameters | None = None,
) -> dict[str, object]:
    params = parameters or TimelineParameters()
    return evaluate_artifact_events_timeline(
        _filtered_events(root, params),
        artifact_dir=root,
        parameters=params,
        event_source="artifact-scan",
    )


def evaluate_artifact_events_timeline(
    events: tuple[ArtifactEvent, ...],
    *,
    artifact_dir: Path,
    parameters: TimelineParameters | None = None,
    event_source: str = "artifact-scan",
) -> dict[str, object]:
    params = parameters or TimelineParameters()
    context = _timeline_context(events, params)
    return _timeline_report(artifact_dir, params, context, event_source=event_source)


def _timeline_context(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> TimelineContext:
    sessions, events = _selected_sessions(events, params)
    session_rows = [_session_row(session, params) for session in sessions]
    burst_rows = [
        burst for session in sessions for burst in _microburst_rows(session, params)
    ]
    repeat_groups = _repeat_groups(events, None)
    fanout_hotspots = _fanout_hotspots(burst_rows, None)
    prime_suppression = prime_suppression_candidates(
        sessions,
        limit=params.examples,
    )
    fzf_promotion = fzf_promotion_candidates(
        repeat_groups,
        limit=params.examples,
    )
    owner_collapse = owner_collapse_candidates(
        repeat_groups,
        limit=params.examples,
    )
    fanout_planning = fanout_planning_candidates(
        fanout_hotspots,
        limit=params.examples,
    )
    read_loop_risk = read_loop_risk_summary(events, limit=params.examples)
    return TimelineContext(
        events=events,
        sessions=sessions,
        session_rows=session_rows,
        burst_rows=burst_rows,
        repeat_groups=repeat_groups,
        fanout_hotspots=fanout_hotspots,
        prime_suppression=prime_suppression,
        fzf_promotion=fzf_promotion,
        owner_collapse=owner_collapse,
        fanout_planning=fanout_planning,
        read_loop_risk=read_loop_risk,
    )


def _filtered_events(
    root: Path, params: TimelineParameters
) -> tuple[ArtifactEvent, ...]:
    events = scan_artifact_events(root)
    if params.since_timestamp is not None:
        return tuple(
            event for event in events if event.timestamp >= params.since_timestamp
        )
    return events


def _selected_sessions(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> tuple[tuple[tuple[ArtifactEvent, ...], ...], tuple[ArtifactEvent, ...]]:
    sessions = _sessions(events, params.session_gap_seconds)
    if params.recent_sessions is not None:
        sessions = sessions[-params.recent_sessions :]
        events = tuple(event for session in sessions for event in session)
    return sessions, events


def _timeline_report(
    root: Path,
    params: TimelineParameters,
    context: TimelineContext,
    *,
    event_source: str,
) -> dict[str, object]:
    report_actions = {
        "primeSuppression": context.prime_suppression,
        "fzfPromotion": context.fzf_promotion,
        "ownerCollapse": context.owner_collapse,
        "fanoutPlanning": context.fanout_planning,
    }
    summary = action_summary(
        report_actions,
        limit=min(3, params.examples),
    )
    report: dict[str, object] = {
        "schemaId": "agent.semantic-protocols.graph-turbo-artifact-timeline",
        "schemaVersion": "1",
        "artifactDir": str(root),
        "eventSource": event_source,
        "parameters": _parameter_row(params),
        "eventCount": len(context.events),
        "actionEventCount": sum(1 for event in context.events if event.action),
        "sessionCount": len(context.sessions),
        "roundCount": len(context.burst_rows),
        "microburstCount": len(context.burst_rows),
        "fanoutBurstCount": sum(
            1 for burst in context.burst_rows if burst["fanoutWidth"] >= 2
        ),
        "softOverrunMicrobursts": _soft_overrun_count(context.burst_rows, params),
        "inferredSubagentStarts": sum(
            int(burst["fanoutWidth"])
            for burst in context.burst_rows
            if int(burst["fanoutWidth"]) >= 2
        ),
        "repeatSearches": sum(
            int(row["repeatSearches"]) for row in context.session_rows
        ),
        "suppressiblePrimeSearches": context.prime_suppression["suppressibleSearches"],
        "promotableFzfSearches": context.fzf_promotion["promotableSearches"],
        "collapsibleOwnerSearches": context.owner_collapse["collapsibleSearches"],
        "routableFanoutBursts": context.fanout_planning["routableFanoutBursts"],
        "avoidableFanoutBranches": context.fanout_planning["avoidableFanoutBranches"],
        "readLoopDirectCodeReads": context.read_loop_risk["directCodeReads"],
        "readLoopDuplicateSelectors": context.read_loop_risk["duplicateSelectors"],
        "readLoopAdjacentRangeWindows": context.read_loop_risk["adjacentRangeWindows"],
        "readLoopSameOwnerScans": context.read_loop_risk["sameOwnerScans"],
        "kindCounts": dict(
            sorted(Counter(event.kind for event in context.events).items())
        ),
        "actionMethodCounts": _action_method_counts(context.events),
        "sessions": context.session_rows[-params.examples :],
        "topMicrobursts": _top_microbursts(context.burst_rows, params.examples),
        "fanoutHotspots": context.fanout_hotspots[: params.examples],
        "repeatGroups": context.repeat_groups[: params.examples],
        "optimizationTargets": optimization_targets(
            context.repeat_groups,
            context.fanout_hotspots,
            limit=params.examples,
        ),
        "primeSuppression": context.prime_suppression,
        "fzfPromotion": context.fzf_promotion,
        "ownerCollapse": context.owner_collapse,
        "fanoutPlanning": context.fanout_planning,
        "readLoopRisk": context.read_loop_risk,
        "actionSummary": summary,
    }
    report["efficiencyEstimate"] = efficiency_estimate(report)
    return report


def _sessions(
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


def _session_row(
    session: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> dict[str, object]:
    bursts = _microburst_rows(session, params)
    reasoning_gaps = _reasoning_gaps(bursts, params.subagent_start_gap_seconds)
    return {
        "start": _timestamp(session[0].timestamp),
        "end": _timestamp(session[-1].timestamp),
        "durationSeconds": round(session[-1].timestamp - session[0].timestamp, 3),
        "events": len(session),
        "actions": sum(1 for event in session if event.action),
        "microbursts": len(bursts),
        "fanoutBursts": sum(1 for burst in bursts if burst["fanoutWidth"] >= 2),
        "softOverrunMicrobursts": _soft_overrun_count(bursts, params),
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


def _microburst_rows(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> list[dict[str, object]]:
    bursts = _microbursts(
        tuple(event for event in events if event.action),
        gap_seconds=params.subagent_start_gap_seconds,
        hard_max_seconds=params.subagent_hard_max_seconds,
    )
    return [_burst_row(burst) for burst in bursts]


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
    keys = {_fanout_key(event) for event in burst}
    return {
        "start": _timestamp(burst[0].timestamp),
        "end": _timestamp(burst[-1].timestamp),
        "spanSeconds": round(burst[-1].timestamp - burst[0].timestamp, 3),
        "events": len(burst),
        "fanoutWidth": len(keys),
        "commands": sum(1 for event in burst if event.kind == "command"),
        "searches": sum(1 for event in burst if event.kind == "search"),
        "queries": sum(1 for event in burst if event.kind == "query"),
        "methods": dict(sorted(Counter(event.method for event in burst).items())),
        "fanoutKeys": [_key_row(key) for key in sorted(keys)],
        "examples": [_event_example(event) for event in burst[:5]],
    }


def _repeat_searches(events: tuple[ArtifactEvent, ...]) -> int:
    counts = Counter(
        _fanout_key(event)
        for event in events
        if event.action and event.method.startswith("search/")
    )
    return sum(count - 1 for count in counts.values() if count > 1)


def _repeat_groups(
    events: tuple[ArtifactEvent, ...], examples: int | None
) -> list[dict[str, object]]:
    groups: dict[tuple[str, str, str, str], list[ArtifactEvent]] = {}
    for event in events:
        if event.action and event.method.startswith("search/"):
            groups.setdefault(_fanout_key(event), []).append(event)
    rows = [
        {
            **_key_row(key),
            "count": len(items),
            "repeatCount": len(items) - 1,
            "first": _timestamp(items[0].timestamp),
            "last": _timestamp(items[-1].timestamp),
            "spanSeconds": round(items[-1].timestamp - items[0].timestamp, 3),
            "examples": [_event_example(event) for event in items[:3]],
        }
        for key, items in sorted(
            groups.items(),
            key=lambda item: (
                -(len(item[1]) - 1),
                -(item[1][-1].timestamp - item[1][0].timestamp),
                item[0],
            ),
        )
        if len(items) > 1
    ]
    return rows if examples is None else rows[:examples]


def _reasoning_gaps(bursts: list[dict[str, object]], gap_seconds: int) -> list[float]:
    gaps: list[float] = []
    for previous, current in zip(bursts, bursts[1:]):
        gap = _parse_time(str(current["start"])) - _parse_time(str(previous["end"]))
        if gap > gap_seconds:
            gaps.append(round(gap, 3))
    return gaps


def _top_microbursts(
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


def _fanout_hotspots(
    bursts: list[dict[str, object]], examples: int | None
) -> list[dict[str, object]]:
    rows = [
        _hotspot_row(burst)
        for burst in sorted(
            (burst for burst in bursts if int(burst["fanoutWidth"]) >= 2),
            key=lambda burst: (
                -int(burst["fanoutWidth"]),
                -int(burst["events"]),
                float(burst["spanSeconds"]),
            ),
        )
    ]
    return rows if examples is None else rows[:examples]


def _hotspot_row(burst: dict[str, object]) -> dict[str, object]:
    return {
        "start": burst["start"],
        "end": burst["end"],
        "spanSeconds": burst["spanSeconds"],
        "events": burst["events"],
        "fanoutWidth": burst["fanoutWidth"],
        "methods": burst["methods"],
        "fanoutKeys": burst["fanoutKeys"],
    }


def _soft_overrun_count(
    bursts: list[dict[str, object]], params: TimelineParameters
) -> int:
    return sum(
        1
        for burst in bursts
        if float(burst["spanSeconds"]) > params.subagent_soft_max_seconds
    )


def _action_method_counts(events: tuple[ArtifactEvent, ...]) -> dict[str, int]:
    counts = Counter(
        f"{event.language}:{event.method}" for event in events if event.action
    )
    return dict(sorted(counts.items()))


def _fanout_key(event: ArtifactEvent) -> tuple[str, str, str, str]:
    subject = event.target or event.query or event.path.rsplit("/", 1)[-1]
    return (event.language, event.method, subject, event.project_root_arg)


def _key_row(key: tuple[str, str, str, str]) -> dict[str, str]:
    language, method, subject, project_root_arg = key
    row = {
        "language": language,
        "method": method,
        "subject": subject,
    }
    if project_root_arg:
        row["projectRootArg"] = project_root_arg
    return row


def _event_example(event: ArtifactEvent) -> dict[str, object]:
    row = {
        "kind": event.kind,
        "language": event.language,
        "method": event.method,
        "target": event.target or event.query,
        "path": event.path,
    }
    if event.project_root_arg:
        row["projectRootArg"] = event.project_root_arg
    return row


def _parameter_row(params: TimelineParameters) -> dict[str, object]:
    return {
        "subagentStartGapSeconds": params.subagent_start_gap_seconds,
        "subagentSoftMaxSeconds": params.subagent_soft_max_seconds,
        "subagentHardMaxSeconds": params.subagent_hard_max_seconds,
        "sessionGapSeconds": params.session_gap_seconds,
        "since": (
            _timestamp(params.since_timestamp)
            if params.since_timestamp is not None
            else None
        ),
        "recentSessions": params.recent_sessions,
    }


def _timestamp(value: float) -> str:
    return datetime.fromtimestamp(value).isoformat(timespec="seconds")


def _parse_time(value: str) -> float:
    return datetime.fromisoformat(value).timestamp()
