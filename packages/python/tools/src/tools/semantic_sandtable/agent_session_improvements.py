"""Build reporting-oriented improvement data from agent-session analysis."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .utils import dict_value, list_value, optional_int, require_str


def build_agent_session_improvement_report(
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    *,
    source_quality_report_path: str,
    source_graph_turbo_feedback_path: str,
) -> dict[str, Any]:
    return {
        "schemaId": (
            "agent.semantic-protocols.semantic-agent-session-improvement-report"
        ),
        "schemaVersion": "1",
        "sessionId": require_str(quality_report, "sessionId", "unknown"),
        "scenarioId": require_str(
            quality_report,
            "scenarioId",
            "recorded.agent-session",
        ),
        "sourceQualityReportPath": source_quality_report_path,
        "sourceGraphTurboFeedbackPath": source_graph_turbo_feedback_path,
        "metrics": _report_metrics(quality_report, graph_turbo_feedback),
        "improvementPoints": _improvement_points(
            quality_report,
            graph_turbo_feedback,
        ),
    }


def write_agent_session_improvement_report(
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    output_path: Path,
    *,
    source_quality_report_path: str,
    source_graph_turbo_feedback_path: str,
) -> dict[str, Any]:
    report = build_agent_session_improvement_report(
        quality_report,
        graph_turbo_feedback,
        source_quality_report_path=source_quality_report_path,
        source_graph_turbo_feedback_path=source_graph_turbo_feedback_path,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return report


def _report_metrics(
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
) -> dict[str, Any]:
    summary = dict_value(quality_report.get("summary"))
    round_summary = dict_value(quality_report.get("roundSummary"))
    answer = dict_value(quality_report.get("answer"))
    return {
        "commandCount": optional_int(summary.get("commandCount")) or 0,
        "aspCommands": optional_int(summary.get("aspCommands")) or 0,
        "searchCommands": optional_int(summary.get("searchCommands")) or 0,
        "queryCommands": optional_int(summary.get("queryCommands")) or 0,
        "directReadRiskCommands": optional_int(
            summary.get("directReadRiskCommands")
        )
        or 0,
        "repeatedCommands": optional_int(summary.get("repeatedCommands")) or 0,
        "deniedCommands": optional_int(summary.get("deniedCommands")) or 0,
        "stdoutBytes": optional_int(summary.get("stdoutBytes")) or 0,
        "stderrBytes": optional_int(summary.get("stderrBytes")) or 0,
        "totalRounds": optional_int(round_summary.get("totalRounds")) or 0,
        "findingCount": len(list_value(quality_report.get("findings"))),
        "graphTurboCandidateCount": len(
            list_value(graph_turbo_feedback.get("candidates"))
        ),
        "roundStatusCounts": _round_status_counts(quality_report),
        "commandKindCounts": dict_value(round_summary.get("commandKindCounts")),
        "qualitySignalCounts": dict_value(round_summary.get("qualitySignalCounts")),
        "answer": _answer_metrics(answer),
    }


def _improvement_points(
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
) -> list[dict[str, Any]]:
    points = []
    for finding in list_value(quality_report.get("findings")):
        if isinstance(finding, dict):
            points.append(_point_from_finding(finding, quality_report))
    for candidate in list_value(graph_turbo_feedback.get("candidates")):
        if isinstance(candidate, dict):
            points.append(_point_from_candidate(candidate))
    return [point for point in points if point]


def _point_from_finding(
    finding: dict[str, Any],
    quality_report: dict[str, Any],
) -> dict[str, Any]:
    finding_id = require_str(finding, "id", "finding")
    metric, value, target = _finding_metric(finding_id, quality_report)
    return {
        "id": f"improve.{finding_id}",
        "category": _finding_category(finding),
        "severity": require_str(finding, "severity", "warning"),
        "title": require_str(finding, "message", finding_id),
        "observed": {"metric": metric, "value": value},
        "target": {"metric": metric, "value": target},
        "evidenceRefs": _finding_evidence_refs(finding_id, quality_report),
        "recommendedAction": require_str(
            finding,
            "recommendedAction",
            "Review the source quality finding.",
        ),
        "expectedImpact": _finding_expected_impact(finding_id),
        "sourceFindingIds": [finding_id],
    }


def _point_from_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    candidate_id = require_str(candidate, "id", "candidate")
    candidate_kind = require_str(candidate, "kind", "missing-fact")
    return {
        "id": f"improve.{candidate_id}",
        "category": "graph-turbo",
        "severity": "warning",
        "title": require_str(candidate, "reason", candidate_kind),
        "observed": {"metric": "graphTurboCandidate", "value": candidate_kind},
        "target": {
            "metric": "graphTurboCandidate",
            "value": require_str(
                candidate,
                "expectedChange",
                "reviewed-calibration-input",
            ),
        },
        "evidenceRefs": [str(item) for item in list_value(candidate.get("evidenceRefs"))],
        "recommendedAction": require_str(
            candidate,
            "recommendedAction",
            "Review graph-turbo feedback before calibration.",
        ),
        "expectedImpact": "Make graph-turbo calibration reviewable from trace data.",
        "sourceCandidateIds": [candidate_id],
    }


def _finding_metric(
    finding_id: str,
    quality_report: dict[str, Any],
) -> tuple[str, int | str | bool, int | str | bool]:
    metrics = _report_metrics(quality_report, {"candidates": []})
    answer = dict_value(metrics.get("answer"))
    if finding_id == "answer.missing":
        return "answer.present", bool(answer.get("present")), True
    if finding_id == "answer.weak-grounding":
        return "answer.groundingStatus", str(answer.get("groundingStatus")), "grounded"
    if finding_id == "search.missing-prime":
        return "searchPrimeCommands", 0, ">=1-before-followup"
    if finding_id == "read.direct-risk":
        return "directReadRiskCommands", metrics["directReadRiskCommands"], 0
    if finding_id == "command.repeated":
        return "repeatedCommands", metrics["repeatedCommands"], 0
    if finding_id == "hook.denied":
        return "deniedCommands", metrics["deniedCommands"], 0
    return "findingCount", metrics["findingCount"], 0


def _finding_category(finding: dict[str, Any]) -> str:
    kind = finding.get("kind")
    if kind == "answer-grounding":
        return "answer-grounding"
    if kind == "search-flow":
        return "search-flow"
    if kind == "hook-follow":
        return "hook-follow"
    return "command-efficiency"


def _finding_evidence_refs(
    finding_id: str,
    quality_report: dict[str, Any],
) -> list[str]:
    refs = []
    for round_detail in list_value(quality_report.get("roundDetails")):
        if not isinstance(round_detail, dict):
            continue
        if finding_id in list_value(round_detail.get("findingIds")):
            refs.append(require_str(round_detail, "id", "round"))
    if refs:
        return refs
    for turn_detail in list_value(quality_report.get("turnDetails")):
        if not isinstance(turn_detail, dict):
            continue
        if finding_id in list_value(turn_detail.get("findingIds")):
            refs.append(require_str(turn_detail, "id", "turn"))
    return refs


def _finding_expected_impact(finding_id: str) -> str:
    return {
        "answer.missing": "Improve completion quality and answer auditability.",
        "answer.weak-grounding": "Make final answers easier to defend from evidence.",
        "search.missing-prime": "Reduce low-quality search starts and wasted follow-up.",
        "read.direct-risk": "Reduce broad source reads before parser-owned evidence.",
        "command.repeated": "Reduce repeated command rounds through query-set guidance.",
        "hook.denied": "Improve hook-follow behavior after denied commands.",
    }.get(finding_id, "Make the improvement point reviewable from trace data.")


def _answer_metrics(answer: dict[str, Any]) -> dict[str, Any]:
    return {
        "present": bool(answer.get("present")),
        "groundingStatus": require_str(answer, "groundingStatus", "unknown"),
        "afterLastToolUse": bool(answer.get("afterLastToolUse")),
        "textBytes": optional_int(answer.get("textBytes")) or 0,
        "textLineCount": optional_int(answer.get("textLineCount")) or 0,
    }


def _round_status_counts(quality_report: dict[str, Any]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for round_detail in list_value(quality_report.get("roundDetails")):
        if not isinstance(round_detail, dict):
            continue
        status = require_str(round_detail, "resultStatus", "unknown")
        counts[status] = counts.get(status, 0) + 1
    return dict(sorted(counts.items()))
