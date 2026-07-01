"""Assemble artifact timeline analysis context from projected event rows."""

from __future__ import annotations

from dataclasses import dataclass

from .artifact_events import ArtifactEvent
from .artifact_fanout_planning import fanout_planning_candidates
from .artifact_owner_collapse import owner_collapse_candidates
from .artifact_prime_suppression import prime_suppression_candidates
from .artifact_read_loop import read_loop_risk_summary
from .artifact_timeline_parameters import TimelineParameters
from .artifact_timeline_repeats import fanout_hotspots, repeat_groups
from .artifact_timeline_sessions import microburst_rows, selected_sessions, session_row
from .artifact_topology import topology_summary
from .artifact_typed_frontier_promotion import typed_frontier_promotion_candidates


@dataclass(frozen=True)
class TimelineContext:
    events: tuple[ArtifactEvent, ...]
    sessions: tuple[tuple[ArtifactEvent, ...], ...]
    session_rows: list[dict[str, object]]
    burst_rows: list[dict[str, object]]
    repeat_groups: list[dict[str, object]]
    fanout_hotspots: list[dict[str, object]]
    prime_suppression: dict[str, object]
    typed_frontier_promotion: dict[str, object]
    owner_collapse: dict[str, object]
    fanout_planning: dict[str, object]
    read_loop_risk: dict[str, object]
    topology: dict[str, object]


def timeline_context(
    events: tuple[ArtifactEvent, ...], params: TimelineParameters
) -> TimelineContext:
    sessions, events = selected_sessions(events, params)
    session_rows = [session_row(session, params) for session in sessions]
    burst_rows = [
        burst for session in sessions for burst in microburst_rows(session, params)
    ]
    repeated_searches = repeat_groups(events, None)
    hotspots = fanout_hotspots(burst_rows, None)
    prime_suppression = prime_suppression_candidates(
        sessions,
        limit=params.examples,
    )
    typed_frontier_promotion = typed_frontier_promotion_candidates(
        repeated_searches,
        limit=params.examples,
    )
    owner_collapse = owner_collapse_candidates(
        repeated_searches,
        limit=params.examples,
    )
    fanout_planning = fanout_planning_candidates(
        hotspots,
        limit=params.examples,
    )
    read_loop_risk = read_loop_risk_summary(events, limit=params.examples)
    topology = topology_summary(events, limit=params.examples)
    return TimelineContext(
        events=events,
        sessions=sessions,
        session_rows=session_rows,
        burst_rows=burst_rows,
        repeat_groups=repeated_searches,
        fanout_hotspots=hotspots,
        prime_suppression=prime_suppression,
        typed_frontier_promotion=typed_frontier_promotion,
        owner_collapse=owner_collapse,
        fanout_planning=fanout_planning,
        read_loop_risk=read_loop_risk,
        topology=topology,
    )
