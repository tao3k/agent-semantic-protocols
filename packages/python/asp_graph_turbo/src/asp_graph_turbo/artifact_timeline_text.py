"""Text rendering for graph turbo artifact timeline reports."""

from __future__ import annotations

import sys


def write_timeline_text_report(report: dict[str, object]) -> None:
    for line in timeline_text_lines(report):
        sys.stdout.write(line + "\n")


def timeline_text_lines(report: dict[str, object]) -> tuple[str, ...]:
    return tuple(
        line for section in _timeline_line_sections(report) for line in section
    )


def _timeline_line_sections(report: dict[str, object]) -> tuple[tuple[str, ...], ...]:
    return (
        _summary_lines(report),
        _burst_lines(report),
        _repeat_lines(report),
        _target_lines(report),
        _efficiency_lines(report.get("efficiencyEstimate")),
        _read_loop_lines(report.get("readLoopRisk")),
        _topology_lines(report.get("topology")),
        _action_summary_lines(report.get("actionSummary")),
        _suppression_lines(report.get("primeSuppression")),
        _promotion_lines(report.get("typedFrontierPromotion")),
        _collapse_lines(report.get("ownerCollapse")),
        _fanout_lines(report.get("fanoutPlanning")),
        _session_lines(report),
    )


def _summary_lines(report: dict[str, object]) -> tuple[str, ...]:
    return (
        "[graph-turbo-timeline] "
        f"source={report['eventSource']} "
        f"events={report['eventCount']} actions={report['actionEventCount']} "
        f"sessions={report['sessionCount']} rounds={report['roundCount']} "
        f"microbursts={report['microburstCount']} "
        f"fanoutBursts={report['fanoutBurstCount']} "
        f"softOverrunMicrobursts={report['softOverrunMicrobursts']} "
        f"inferredSubagentStarts={report['inferredSubagentStarts']} "
        f"repeatSearches={report['repeatSearches']} "
        f"suppressiblePrimeSearches={report['suppressiblePrimeSearches']} "
        f"promotableTypedFrontierSearches={report['promotableTypedFrontierSearches']} "
        f"collapsibleOwnerSearches={report['collapsibleOwnerSearches']} "
        f"routableFanoutBursts={report['routableFanoutBursts']} "
        f"avoidableFanoutBranches={report['avoidableFanoutBranches']} "
        f"topologyEvents={report.get('topologyEventCount', 0)} "
        f"topologyWeakQueryPackEvents="
        f"{report.get('topologyWeakQueryPackEvents', 0)} "
        f"topologyWeakLocalEvidenceEvents="
        f"{report.get('topologyWeakLocalEvidenceEvents', 0)}",
        f"parameters={report['parameters']}",
        f"kinds={report['kindCounts']}",
        f"actions={report['actionMethodCounts']}",
    )


def _burst_lines(report: dict[str, object]) -> tuple[str, ...]:
    lines = [_microburst_line(burst) for burst in report.get("topMicrobursts", [])]
    lines.extend(_fanout_hotspot_line(row) for row in report.get("fanoutHotspots", []))
    return tuple(lines)


def _microburst_line(burst: dict[str, object]) -> str:
    return (
        "[graph-turbo-microburst] "
        f"start={burst['start']} span={burst['spanSeconds']} "
        f"events={burst['events']} fanout={burst['fanoutWidth']} "
        f"commands={burst['commands']} searches={burst['searches']} "
        f"queries={burst['queries']} methods={burst['methods']}"
    )


def _fanout_hotspot_line(hotspot: dict[str, object]) -> str:
    return (
        "[graph-turbo-fanout] "
        f"start={hotspot['start']} span={hotspot['spanSeconds']} "
        f"fanout={hotspot['fanoutWidth']} events={hotspot['events']} "
        f"methods={hotspot['methods']}"
    )


def _repeat_lines(report: dict[str, object]) -> tuple[str, ...]:
    return tuple(_repeat_line(group) for group in report.get("repeatGroups", []))


def _repeat_line(group: dict[str, object]) -> str:
    root = f" root={group['projectRootArg']}" if group.get("projectRootArg") else ""
    return (
        "[graph-turbo-repeat] "
        f"count={group['count']} repeat={group['repeatCount']} "
        f"language={group['language']} method={group['method']} "
        f"subject={group['subject']}{root} span={group['spanSeconds']}"
    )


def _target_lines(report: dict[str, object]) -> tuple[str, ...]:
    return tuple(
        _target_line(target) for target in report.get("optimizationTargets", [])
    )


def _target_line(target: dict[str, object]) -> str:
    return (
        "[graph-turbo-target] "
        f"category={target['category']} severity={target['severity']} "
        f"score={target['impactScore']} layer={target['layer']} "
        f"evidence={target['evidence']} action={target['nextAction']}"
        f"{_route_suffix(target)}"
    )


def _efficiency_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-efficiency] "
        f"policy={value['policy']} observedActions={value['observedActions']} "
        f"observedRounds={value['observedRounds']} "
        f"repeatSearches={value['repeatSearches']} "
        f"routableFanoutBursts={value['routableFanoutBursts']} "
        f"typedFrontierAvoidableSearches="
        f"{value['typedFrontierAvoidableSearches']} "
        f"avoidableFanoutBranches={value['avoidableFanoutBranches']} "
        f"estimatedAvoidableActionsUpperBound="
        f"{value['estimatedAvoidableActionsUpperBound']} "
        f"estimatedActionReductionRatioUpperBound="
        f"{value['estimatedActionReductionRatioUpperBound']} "
        f"recommendedFirstCommand={value['recommendedFirstCommand']}",
    )


def _action_summary_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-next-summary] "
        f"policy={value['policy']} actions={value['actionCount']} "
        f"replacement={value['replacement']}",
        *(_next_action_line(action) for action in value.get("actions", [])),
    )


def _read_loop_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-read-loop] "
        f"policy={value['policy']} "
        f"directCodeReads={value['directCodeReads']} "
        f"duplicateSelectors={value['duplicateSelectors']} "
        f"adjacentRangeWindows={value['adjacentRangeWindows']} "
        f"sameOwnerScans={value['sameOwnerScans']} "
        f"riskCount={value['riskCount']}",
    )


def _topology_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    lines = [
        "[graph-turbo-topology] "
        f"policy={value['policy']} "
        f"events={value['eventsWithTopology']} "
        f"weakQueryPack={value['weakQueryPackEvents']} "
        f"weakPackageCohesion={value['weakPackageCohesionEvents']} "
        f"weakScope={value['weakScopeEvents']} "
        f"weakLocalEvidence={value['weakLocalEvidenceEvents']} "
        f"missingRoutes={value['missingRouteEvents']} "
        f"missingAxes={value['missingAxisCounts']}"
    ]
    lines.extend(_topology_state_line(state) for state in value.get("states", []))
    lines.extend(_topology_action_line(action) for action in value.get("actions", []))
    return tuple(lines)


def _topology_state_line(state: dict[str, object]) -> str:
    return (
        "[graph-turbo-topology-state] "
        f"kind={state['kind']} method={state['method']} "
        f"queryQuality={state['queryQuality']} "
        f"scopeQuality={state['scopeQuality']} "
        f"packageCohesion={state['packageCohesion']} "
        f"owners={state['ownerCandidateCount']} "
        f"rankedEvidence={state['rankedEvidenceCount']} "
        f"route={state['recommendedRoute']} "
        f"missingAxes={state['missingAxes']}"
    )


def _topology_action_line(action: dict[str, object]) -> str:
    if action.get("axis"):
        subject = f"axis={action['axis']}"
    else:
        subject = f"route={action.get('route', '')}"
    return (
        "[graph-turbo-topology-action] "
        f"decision={action['decision']} {subject} "
        f"count={action['count']} replacement={action['replacement']}"
    )


def _next_action_line(action: dict[str, object]) -> str:
    subject = (
        action.get("subject")
        or action.get("owner")
        or action.get("query")
        or action.get("method")
        or ""
    )
    root = f" root={action['projectRootArg']}" if action.get("projectRootArg") else ""
    return (
        "[graph-turbo-next] "
        f"source={action['source']} category={action['category']} "
        f"score={action['impactScore']} decision={action['decision']} "
        f"subject={subject}{root} replacement={action['replacement']}"
        f"{_route_suffix(action)}"
    )


def _suppression_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    lines = [
        "[graph-turbo-prime-suppression] "
        f"policy={value['policy']} "
        f"suppressible={value['suppressibleSearches']} "
        f"groups={value['candidateGroupCount']} "
        f"actions={value['actionCount']} "
        f"replacement={value['replacement']}"
    ]
    lines.extend(_prime_group_line(group) for group in value.get("candidateGroups", []))
    lines.extend(_prime_action_line(action) for action in value.get("actions", []))
    return tuple(lines)


def _prime_group_line(group: dict[str, object]) -> str:
    return (
        "[graph-turbo-prime-group] "
        f"language={group['language']} subject={group['subject']} "
        f"count={group['count']} suppressible={group['suppressibleSearches']} "
        f"span={group['spanSeconds']}"
    )


def _prime_action_line(action: dict[str, object]) -> str:
    return (
        "[graph-turbo-prime-action] "
        f"decision={action['decision']} "
        f"language={action['language']} subject={action['subject']} "
        f"age={action['ageSeconds']} replacement={action['replacement']}"
    )


def _promotion_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-typed-frontier-promotion] "
        f"policy={value['policy']} promotable={value['promotableSearches']} "
        f"groups={value['candidateGroupCount']} actions={value['actionCount']} "
        f"replacement={value['replacement']}",
        *(_lexical_action_line(action) for action in value.get("actions", [])),
    )


def _lexical_action_line(action: dict[str, object]) -> str:
    root = f" root={action['projectRootArg']}" if action.get("projectRootArg") else ""
    return (
        "[graph-turbo-typed-frontier-action] "
        f"decision={action['decision']} "
        f"language={action['language']} query={action['query']} "
        f"repeat={action['repeatCount']}{root} "
        f"replacement={action['replacement']}{_route_suffix(action)}"
    )


def _collapse_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-owner-collapse] "
        f"policy={value['policy']} collapsible={value['collapsibleSearches']} "
        f"groups={value['candidateGroupCount']} actions={value['actionCount']} "
        f"replacement={value['replacement']}",
        *(_owner_action_line(action) for action in value.get("actions", [])),
    )


def _owner_action_line(action: dict[str, object]) -> str:
    root = f" root={action['projectRootArg']}" if action.get("projectRootArg") else ""
    return (
        "[graph-turbo-owner-action] "
        f"decision={action['decision']} "
        f"language={action['language']} owner={action['owner']} "
        f"repeat={action['repeatCount']}{root} "
        f"replacement={action['replacement']}{_route_suffix(action)}"
    )


def _fanout_lines(value: object) -> tuple[str, ...]:
    if not isinstance(value, dict):
        return ()
    return (
        "[graph-turbo-fanout-planning] "
        f"policy={value['policy']} routable={value['routableFanoutBursts']} "
        f"avoidableBranches={value['avoidableFanoutBranches']} "
        f"groups={value['candidateGroupCount']} actions={value['actionCount']} "
        f"replacement={value['replacement']}",
        *(_fanout_action_line(action) for action in value.get("actions", [])),
    )


def _fanout_action_line(action: dict[str, object]) -> str:
    root = f" root={action['projectRootArg']}" if action.get("projectRootArg") else ""
    return (
        "[graph-turbo-fanout-action] "
        f"decision={action['decision']} "
        f"language={action['language']} method={action['method']} "
        f"subject={action['subject']} fanout={action['fanoutWidth']} "
        f"events={action['events']}{root} "
        f"replacement={action['replacement']}{_route_suffix(action)}"
    )


def _session_lines(report: dict[str, object]) -> tuple[str, ...]:
    return tuple(_session_line(session) for session in report.get("sessions", []))


def _session_line(session: dict[str, object]) -> str:
    return (
        "[graph-turbo-session] "
        f"start={session['start']} end={session['end']} "
        f"duration={session['durationSeconds']} events={session['events']} "
        f"actions={session['actions']} microbursts={session['microbursts']} "
        f"fanoutBursts={session['fanoutBursts']} "
        f"repeatSearches={session['repeatSearches']}"
    )


def _route_suffix(row: object) -> str:
    if not isinstance(row, dict):
        return ""
    parts = []
    if row.get("profile"):
        parts.append(f"profile={row['profile']}")
    if row.get("route"):
        parts.append(f"route={row['route']}")
    if row.get("preferredCommand"):
        parts.append(f"preferredCommand={row['preferredCommand']}")
    return " " + " ".join(parts) if parts else ""
