"""Analyze agent-session receipts and emit graph-turbo feedback candidates."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .agent_session_turns import (
    agent_session_round_details,
    agent_session_round_summary,
    agent_session_turn_details,
    agent_session_turn_summary,
)
from .agent_session_improvements import write_agent_session_improvement_report
from .agent_session_question_plan import write_agent_session_question_plan
from .agent_session_algorithm_feedback import (
    write_graph_turbo_algorithm_feedback,
    write_graph_turbo_calibration_proposal,
)
from .agent_session_graph_turbo_events import (
    graph_turbo_request_packet_from_events,
    graph_turbo_seed_plan_candidates_from_events,
)
from .agent_session_quality_findings import candidate_from_finding, quality_findings
from .utils import dict_value, list_value, require_str


def analyze_agent_session_receipt(
    receipt: dict[str, Any],
    *,
    events: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    event_items = events or []
    findings = quality_findings(receipt, event_items)
    turn_details = agent_session_turn_details(receipt, event_items, findings)
    round_details = agent_session_round_details(turn_details)
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-quality-report",
        "schemaVersion": "1",
        "sessionId": require_str(receipt, "sessionId", "unknown"),
        "scenarioId": require_str(receipt, "scenarioId", "recorded.agent-session"),
        "summary": dict_value(receipt.get("summary")),
        "answer": dict_value(receipt.get("answer")),
        "findings": findings,
        "turnSummary": agent_session_turn_summary(turn_details),
        "turnDetails": turn_details,
        "roundSummary": agent_session_round_summary(round_details),
        "roundDetails": round_details,
    }


def graph_turbo_feedback_from_analysis(
    receipt: dict[str, Any],
    quality_report: dict[str, Any],
    *,
    source_receipt_path: str,
    events: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    candidates = []
    for finding in list_value(quality_report.get("findings")):
        if not isinstance(finding, dict):
            continue
        candidate = candidate_from_finding(finding)
        if candidate:
            candidates.append(candidate)
    candidates.extend(graph_turbo_seed_plan_candidates_from_events(receipt, events or []))
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-graph-turbo-feedback",
        "schemaVersion": "1",
        "sessionId": require_str(receipt, "sessionId", "unknown"),
        "scenarioId": require_str(receipt, "scenarioId", "recorded.agent-session"),
        "sourceReceiptPath": source_receipt_path,
        "candidates": candidates,
    }


def write_agent_session_analysis(
    receipt_path: Path,
    quality_report_path: Path,
    feedback_path: Path,
    improvement_report_path: Path | None = None,
    algorithm_feedback_path: Path | None = None,
    algorithm_calibration_path: Path | None = None,
    question_plan_path: Path | None = None,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    events = _load_events_for_receipt(receipt_path, receipt)
    quality = analyze_agent_session_receipt(receipt, events=events)
    feedback = graph_turbo_feedback_from_analysis(
        receipt,
        quality,
        source_receipt_path=str(receipt_path),
        events=events,
    )
    _write_json(quality_report_path, quality)
    _write_json(feedback_path, feedback)
    improvement = {}
    if improvement_report_path is not None:
        improvement = write_agent_session_improvement_report(
            quality,
            feedback,
            improvement_report_path,
            source_quality_report_path=str(quality_report_path),
            source_graph_turbo_feedback_path=str(feedback_path),
        )
        if question_plan_path is not None:
            write_agent_session_question_plan(
                receipt,
                quality,
                feedback,
                improvement,
                events,
                question_plan_path,
                source_receipt_path=str(receipt_path),
                source_quality_report_path=str(quality_report_path),
                source_graph_turbo_feedback_path=str(feedback_path),
                source_improvement_report_path=str(improvement_report_path),
            )
        if algorithm_feedback_path is not None:
            algorithm_feedback = write_graph_turbo_algorithm_feedback(
                improvement,
                feedback,
                algorithm_feedback_path,
                source_path=str(improvement_report_path),
            )
            if algorithm_calibration_path is not None:
                write_graph_turbo_calibration_proposal(
                    algorithm_feedback,
                    algorithm_calibration_path,
                    request_packet=graph_turbo_request_packet_from_events(
                        receipt,
                        events,
                    ),
                )
    return quality, feedback, improvement


def _load_events_for_receipt(
    receipt_path: Path,
    receipt: dict[str, Any],
) -> list[dict[str, Any]]:
    artifact_root = receipt.get("artifactRoot")
    candidates = []
    if isinstance(artifact_root, str) and artifact_root:
        candidates.append(Path(artifact_root) / "events.jsonl")
    candidates.append(receipt_path.parent.parent / "events.jsonl")
    for path in candidates:
        if path.is_file():
            return _load_jsonl(path)
    return []


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


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    items = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                items.append(value)
    return items
