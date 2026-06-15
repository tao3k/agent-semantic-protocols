"""Build quality findings and graph-turbo candidates for agent sessions."""

from __future__ import annotations

from typing import Any

from .agent_session_search_flow import search_flow_findings_from_events
from .utils import dict_value, list_value, optional_int


def quality_findings(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    summary = dict_value(receipt.get("summary"))
    answer = dict_value(receipt.get("answer"))
    findings: list[dict[str, Any]] = []
    if not answer.get("present"):
        findings.append(
            _finding(
                "answer.missing",
                "answer-grounding",
                "error",
                "No final answer was emitted after the last visible tool use.",
                "Ensure the run reaches an answer.final event before analysis.",
            )
        )
    elif answer.get("groundingStatus") in {"weak", "ungrounded", "unknown"}:
        findings.append(
            _finding(
                "answer.weak-grounding",
                "answer-grounding",
                "warning",
                "Final answer is not strongly linked to provider or command evidence.",
                "Attach command, selector, packet, or failure-frontier evidence refs.",
                graph_turbo_feedback=(
                    "Improve frontier evidence that agents can cite in final answers."
                ),
            )
        )
    if optional_int(summary.get("searchCommands")) and not optional_int(
        summary.get("searchPrimeCommands")
    ):
        findings.append(
            _finding(
                "search.missing-prime",
                "search-flow",
                "warning",
                "Search commands ran without a recorded search prime command.",
                "Run search prime before pipe/follow-up searches in live sandtables.",
                graph_turbo_feedback="Prime output may need clearer first-command guidance.",
            )
        )
    if optional_int(summary.get("directReadRiskCommands")):
        findings.append(
            _finding(
                "read.direct-risk",
                "command-efficiency",
                "warning",
                "Broad or unbounded direct-source-read commands were recorded.",
                "Prefer parser-owned search/query selectors before source windows.",
                graph_turbo_feedback=(
                    "Boost exact selector and read-plan actions over broad read routes."
                ),
            )
        )
    if optional_int(summary.get("repeatedCommands")):
        findings.append(
            _finding(
                "command.repeated",
                "command-efficiency",
                "warning",
                "Repeated command argv were recorded in the session.",
                "Merge repeated searches into query-set or pipe composition.",
                graph_turbo_feedback=(
                    "Promote repeated query groups into query-set recommendations."
                ),
            )
        )
    if optional_int(summary.get("deniedCommands")):
        findings.append(
            _finding(
                "hook.denied",
                "hook-follow",
                "info",
                "Hook denials were recorded and should be audited for safe-route follow-up.",
                "Check that the next accepted command follows the hook guidance.",
            )
        )
    findings.extend(search_flow_findings_from_events(receipt, events))
    return findings


def candidate_from_finding(finding: dict[str, Any]) -> dict[str, Any] | None:
    kind = finding.get("kind")
    if kind == "command-efficiency" and finding.get("id") == "command.repeated":
        candidate_kind = "repeated-query-group"
    elif finding.get("id") == "search.path-intent-lost":
        candidate_kind = "path-intent-lost"
    elif finding.get("id") == "search.finder-path-ignored":
        candidate_kind = "finder-path-ignored"
    elif finding.get("id") == "search.package-drift":
        candidate_kind = "search-flow-drift"
    elif kind == "search-flow":
        candidate_kind = "unclear-next-action"
    elif kind == "answer-grounding":
        candidate_kind = "missing-fact"
    else:
        return None
    candidate = {
        "id": f"gt.{finding.get('id')}",
        "kind": candidate_kind,
        "confidence": _candidate_confidence(finding, candidate_kind),
        "reason": str(finding.get("graphTurboFeedback") or finding.get("message")),
        "evidenceRefs": [
            str(item) for item in list_value(finding.get("evidenceRefs"))
        ],
        "recommendedAction": str(finding.get("recommendedAction", "")),
    }
    matched_selectors = [
        str(item) for item in list_value(finding.get("matchedSelectors"))
    ]
    if matched_selectors:
        candidate["matchedSelectors"] = matched_selectors
    return candidate


def _candidate_confidence(
    finding: dict[str, Any],
    candidate_kind: str,
) -> float:
    if candidate_kind in {"path-intent-lost", "finder-path-ignored"}:
        return 0.85
    if finding.get("severity") == "warning":
        return 0.65
    return 0.5


def _finding(
    finding_id: str,
    kind: str,
    severity: str,
    message: str,
    recommended_action: str,
    *,
    graph_turbo_feedback: str | None = None,
) -> dict[str, Any]:
    finding = {
        "id": finding_id,
        "kind": kind,
        "severity": severity,
        "message": message,
        "recommendedAction": recommended_action,
    }
    if graph_turbo_feedback:
        finding["graphTurboFeedback"] = graph_turbo_feedback
    return finding
