"""Timeline report assembly for cached ASP artifact events."""

from __future__ import annotations

from collections import Counter
from pathlib import Path

from .artifact_action_summary import action_summary
from .artifact_efficiency import efficiency_estimate
from .artifact_events import ArtifactEvent
from .artifact_timeline_context import TimelineContext, timeline_context
from .artifact_timeline_rows import (
    TimelineParameters,
    action_method_counts,
    filtered_events,
    parameter_row,
    soft_overrun_count,
    top_microbursts,
)
from .artifact_timeline_targets import optimization_targets
from .artifact_topology import hydrate_topology_metadata


def evaluate_artifact_timeline(
    root: Path,
    *,
    parameters: TimelineParameters | None = None,
) -> dict[str, object]:
    params = parameters or TimelineParameters()
    return evaluate_artifact_events_timeline(
        filtered_events(root, params),
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
    events = hydrate_topology_metadata(events, artifact_dir)
    context = timeline_context(events, params)
    return _timeline_report(artifact_dir, params, context, event_source=event_source)


def _timeline_report(
    root: Path,
    params: TimelineParameters,
    context: TimelineContext,
    *,
    event_source: str,
) -> dict[str, object]:
    report_actions = {
        "primeSuppression": context.prime_suppression,
        "typedFrontierPromotion": context.typed_frontier_promotion,
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
        "parameters": parameter_row(params),
        "eventCount": len(context.events),
        "actionEventCount": sum(1 for event in context.events if event.action),
        "sessionCount": len(context.sessions),
        "roundCount": len(context.burst_rows),
        "microburstCount": len(context.burst_rows),
        "fanoutBurstCount": sum(
            1 for burst in context.burst_rows if burst["fanoutWidth"] >= 2
        ),
        "softOverrunMicrobursts": soft_overrun_count(context.burst_rows, params),
        "inferredSubagentStarts": sum(
            int(burst["fanoutWidth"])
            for burst in context.burst_rows
            if int(burst["fanoutWidth"]) >= 2
        ),
        "repeatSearches": sum(
            int(row["repeatSearches"]) for row in context.session_rows
        ),
        "suppressiblePrimeSearches": context.prime_suppression["suppressibleSearches"],
        "promotableTypedFrontierSearches": context.typed_frontier_promotion[
            "promotableSearches"
        ],
        "collapsibleOwnerSearches": context.owner_collapse["collapsibleSearches"],
        "routableFanoutBursts": context.fanout_planning["routableFanoutBursts"],
        "avoidableFanoutBranches": context.fanout_planning["avoidableFanoutBranches"],
        "readLoopDirectCodeReads": context.read_loop_risk["directCodeReads"],
        "readLoopDuplicateSelectors": context.read_loop_risk["duplicateSelectors"],
        "readLoopAdjacentRangeWindows": context.read_loop_risk["adjacentRangeWindows"],
        "readLoopSameOwnerScans": context.read_loop_risk["sameOwnerScans"],
        "topologyEventCount": context.topology["eventsWithTopology"],
        "topologyWeakQueryPackEvents": context.topology["weakQueryPackEvents"],
        "topologyWeakPackageCohesionEvents": context.topology[
            "weakPackageCohesionEvents"
        ],
        "topologyWeakLocalEvidenceEvents": context.topology[
            "weakLocalEvidenceEvents"
        ],
        "kindCounts": dict(
            sorted(Counter(event.kind for event in context.events).items())
        ),
        "actionMethodCounts": action_method_counts(context.events),
        "sessions": context.session_rows[-params.examples :],
        "topMicrobursts": top_microbursts(context.burst_rows, params.examples),
        "fanoutHotspots": context.fanout_hotspots[: params.examples],
        "repeatGroups": context.repeat_groups[: params.examples],
        "optimizationTargets": optimization_targets(
            context.repeat_groups,
            context.fanout_hotspots,
            limit=params.examples,
        ),
        "primeSuppression": context.prime_suppression,
        "typedFrontierPromotion": context.typed_frontier_promotion,
        "ownerCollapse": context.owner_collapse,
        "fanoutPlanning": context.fanout_planning,
        "readLoopRisk": context.read_loop_risk,
        "topology": context.topology,
        "actionSummary": summary,
    }
    report["efficiencyEstimate"] = efficiency_estimate(report)
    return report
