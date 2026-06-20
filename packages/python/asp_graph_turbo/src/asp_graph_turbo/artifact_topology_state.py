"""Topology state summaries for graph turbo artifact timeline reports."""

from __future__ import annotations

import json
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence

from .artifact_event_model import ArtifactEvent
from .artifact_topology_metadata import TOPOLOGY_EVENT_KINDS


def topology_summary(
    events: Iterable[ArtifactEvent], *, limit: int = 5
) -> dict[str, object]:
    event_count = 0
    states: list[dict[str, object]] = []
    route_counts: Counter[str] = Counter()
    axis_counts: Counter[str] = Counter()
    query_quality_counts: Counter[str] = Counter()
    package_cohesion_counts: Counter[str] = Counter()
    scope_quality_counts: Counter[str] = Counter()

    for event in events:
        event_count += 1
        state = topology_state(event)
        if state is None:
            continue
        states.append(state)
        route_counts[str(state.get("recommendedRoute") or "missing")] += 1
        for axis in state.get("missingAxes", []):
            axis_counts[str(axis)] += 1
        query_quality_counts[str(state.get("queryQuality") or "unknown")] += 1
        package_cohesion_counts[str(state.get("packageCohesion") or "unknown")] += 1
        scope_quality_counts[str(state.get("scopeQuality") or "unknown")] += 1

    return {
        "policy": "graph-turbo-first-stage-topology-v1",
        "eventCount": event_count,
        "eventsWithTopology": len(states),
        "weakQueryPackEvents": axis_counts["query-pack"],
        "weakPackageCohesionEvents": axis_counts["package-cohesion"],
        "weakScopeEvents": axis_counts["scope"],
        "weakLocalEvidenceEvents": axis_counts["local-evidence"],
        "missingRouteEvents": axis_counts["route"],
        "queryQualityCounts": _sorted_counter(query_quality_counts),
        "packageCohesionCounts": _sorted_counter(package_cohesion_counts),
        "scopeQualityCounts": _sorted_counter(scope_quality_counts),
        "routeCounts": _sorted_counter(route_counts),
        "missingAxisCounts": _sorted_counter(axis_counts),
        "states": states[:limit],
        "actions": _topology_actions(axis_counts, route_counts, limit=limit),
    }


def topology_state(event: ArtifactEvent) -> dict[str, object] | None:
    if event.kind not in TOPOLOGY_EVENT_KINDS or not event.metadata:
        return None
    query_quality = _quality_value(event.metadata, "queryQuality")
    scope_quality = _quality_value(event.metadata, "scopeQuality")
    package_cohesion = _quality_value(event.metadata, "packageCohesion")
    owner_candidate_count = _sequence_count(event.metadata.get("ownerCandidates"))
    ranked_evidence_count = _sequence_count(event.metadata.get("rankedEvidence"))
    frontier_counts = _frontier_kind_counts(
        event.metadata.get("evidenceFrontier")
        or event.metadata.get("rankedEvidence")
    )
    route = _route_value(event.metadata)
    missing_axes = _missing_axes(
        event.metadata,
        query_quality=query_quality,
        scope_quality=scope_quality,
        package_cohesion=package_cohesion,
        owner_candidate_count=owner_candidate_count,
        frontier_counts=frontier_counts,
        route=route,
    )
    return {
        "path": event.path,
        "kind": event.kind,
        "method": event.method,
        "target": event.target,
        "query": event.query,
        "projectRootArg": event.project_root_arg,
        "queryQuality": query_quality,
        "scopeQuality": scope_quality,
        "packageCohesion": package_cohesion,
        "ownerCandidateCount": owner_candidate_count,
        "rankedEvidenceCount": ranked_evidence_count,
        "frontierKindCounts": _sorted_counter(frontier_counts),
        "recommendedRoute": route,
        "missingAxes": missing_axes,
    }


def _missing_axes(
    metadata: Mapping[str, object],
    *,
    query_quality: str,
    scope_quality: str,
    package_cohesion: str,
    owner_candidate_count: int,
    frontier_counts: Mapping[str, int],
    route: str,
) -> list[str]:
    axes: list[str] = []
    risk = _risk_text(metadata)
    if query_quality in {"low", "review"} or any(
        token in risk
        for token in ("flat-query", "long-field-signatures", "single-flat-or-recall")
    ):
        axes.append("query-pack")
    if scope_quality == "low" or "broad-scope" in risk:
        axes.append("scope")
    if package_cohesion == "low" or "package-drift" in risk:
        axes.append("package-cohesion")
    if owner_candidate_count == 0 and "ownerCandidates" in metadata:
        axes.append("owner-candidates")
    if not any(frontier_counts.get(kind, 0) for kind in ("owner", "syntax", "tests")):
        axes.append("local-evidence")
    if not route:
        axes.append("route")
    return axes


def _topology_actions(
    axis_counts: Counter[str], route_counts: Counter[str], *, limit: int
) -> list[dict[str, object]]:
    actions: list[dict[str, object]] = []
    for axis, count in axis_counts.most_common(limit):
        actions.append(
            {
                "decision": "repair-first-stage-topology",
                "axis": axis,
                "count": count,
                "replacement": _axis_replacement(axis),
            }
        )
    if route_counts:
        route, count = route_counts.most_common(1)[0]
        actions.append(
            {
                "decision": "preserve-topology-route",
                "route": route,
                "count": count,
                "replacement": "record route evidence before next search action",
            }
        )
    return actions[:limit]


def _axis_replacement(axis: str) -> str:
    return {
        "local-evidence": "promote parser/finder-local evidence before path-only owners",
        "owner-candidates": "recover owner candidates before selector expansion",
        "package-cohesion": "split query pack by package-cohesive axes",
        "query-pack": "split flat query into independent evidence clauses",
        "route": "emit selected topology route and expected next evidence",
        "scope": "narrow broad scope before ranking owners",
    }.get(axis, "record missing topology evidence")


def _quality_value(metadata: Mapping[str, object], key: str) -> str:
    value = metadata.get(key)
    if isinstance(value, str):
        return value
    if isinstance(value, Mapping):
        for nested_key in ("quality", "status", "value"):
            nested = value.get(nested_key)
            if isinstance(nested, str):
                return nested
    return "unknown"


def _route_value(metadata: Mapping[str, object]) -> str:
    for key in ("recommendedNext", "nextCommand", "parserIndexNext", "rgScopeNext"):
        value = metadata.get(key)
        if isinstance(value, str) and value:
            return value
    for item in _iter_items(metadata.get("actionRank")):
        value = str(item)
        if value:
            return value
    return ""


def _frontier_kind_counts(value: object) -> Counter[str]:
    counts: Counter[str] = Counter()
    for item in _iter_items(value):
        text = str(item)
        if "." in text:
            counts[text.rsplit(".", 1)[-1]] += 1
        elif ":" in text:
            counts[text.split(":", 1)[0]] += 1
        elif text:
            counts["unknown"] += 1
    return counts


def _sequence_count(value: object) -> int:
    if isinstance(value, str):
        return len([item for item in value.split(",") if item])
    if isinstance(value, Sequence):
        return len(value)
    if isinstance(value, Mapping):
        return len(value)
    return 0


def _iter_items(value: object) -> Iterable[object]:
    if isinstance(value, str):
        return (item.strip() for item in value.split(",") if item.strip())
    if isinstance(value, Mapping):
        return value.values()
    if isinstance(value, Sequence):
        return value
    return ()


def _risk_text(metadata: Mapping[str, object]) -> str:
    value = metadata.get("risk") or metadata.get("seedPlanDetail")
    if isinstance(value, str):
        return value
    if isinstance(value, Mapping | Sequence):
        return json.dumps(value, sort_keys=True)
    return ""


def _sorted_counter(counter: Mapping[str, int]) -> dict[str, int]:
    return dict(sorted(counter.items(), key=lambda item: (-item[1], item[0])))
