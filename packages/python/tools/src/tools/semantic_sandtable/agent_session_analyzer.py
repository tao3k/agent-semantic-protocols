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
from .agent_session_algorithm_feedback import (
    write_graph_turbo_algorithm_feedback,
    write_graph_turbo_calibration_proposal,
)
from .utils import dict_value, list_value, optional_int, require_str


def analyze_agent_session_receipt(
    receipt: dict[str, Any],
    *,
    events: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    findings = _quality_findings(receipt)
    turn_details = agent_session_turn_details(receipt, events or [], findings)
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
        candidate = _candidate_from_finding(finding)
        if candidate:
            candidates.append(candidate)
    candidates.extend(_seed_plan_candidates_from_events(receipt, events or []))
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
                    request_packet=_graph_turbo_request_packet_from_events(
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


def _quality_findings(receipt: dict[str, Any]) -> list[dict[str, Any]]:
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
    return findings


def _candidate_from_finding(finding: dict[str, Any]) -> dict[str, Any] | None:
    kind = finding.get("kind")
    if kind == "command-efficiency" and finding.get("id") == "command.repeated":
        candidate_kind = "repeated-query-group"
    elif kind == "search-flow":
        candidate_kind = "unclear-next-action"
    elif kind == "answer-grounding":
        candidate_kind = "missing-fact"
    else:
        return None
    return {
        "id": f"gt.{finding.get('id')}",
        "kind": candidate_kind,
        "confidence": 0.5,
        "reason": str(finding.get("graphTurboFeedback") or finding.get("message")),
        "evidenceRefs": [
            str(item) for item in list_value(finding.get("evidenceRefs"))
        ],
        "recommendedAction": str(finding.get("recommendedAction", "")),
    }


def _seed_plan_candidates_from_events(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    artifact_root = receipt.get("artifactRoot")
    root = Path(artifact_root) if isinstance(artifact_root, str) and artifact_root else None
    candidates = []
    seen: set[str] = set()
    for event in events:
        if event.get("kind") != "command.result":
            continue
        for packet in _graph_turbo_request_packets(event, root):
            seed_plan = dict_value(packet.get("seedPlan"))
            candidate = _candidate_from_seed_plan(event, packet, seed_plan)
            if not candidate or candidate["id"] in seen:
                continue
            candidates.append(candidate)
            seen.add(candidate["id"])
    return candidates


def _graph_turbo_request_packets(
    event: dict[str, Any],
    artifact_root: Path | None,
) -> list[dict[str, Any]]:
    packets = []
    for text in _event_stdout_texts(event, artifact_root):
        packet = _json_object(text)
        if packet.get("schemaId") == (
            "agent.semantic-protocols.semantic-graph-turbo-request"
        ):
            packets.append(packet)
    return packets


def _graph_turbo_request_packet_from_events(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    artifact_root = receipt.get("artifactRoot")
    root = Path(artifact_root) if isinstance(artifact_root, str) and artifact_root else None
    packets = [
        packet
        for event in events
        if event.get("kind") == "command.result"
        for packet in _graph_turbo_request_packets(event, root)
    ]
    for packet in packets:
        graph = dict_value(packet.get("graph"))
        if list_value(graph.get("nodes")):
            return packet
    return packets[0] if packets else None


def _event_stdout_texts(
    event: dict[str, Any],
    artifact_root: Path | None,
) -> list[str]:
    texts = []
    if isinstance(event.get("preview"), str):
        texts.append(str(event["preview"]))
    if artifact_root is None:
        return texts
    for ref in list_value(event.get("artifactRefs")):
        if not isinstance(ref, dict) or ref.get("kind") != "stdout":
            continue
        path = ref.get("path")
        if not isinstance(path, str) or not path:
            continue
        output_path = artifact_root / path
        if output_path.is_file():
            texts.append(output_path.read_text(encoding="utf-8"))
    return texts


def _json_object(text: str) -> dict[str, Any]:
    try:
        value = json.loads(text.strip())
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def _candidate_from_seed_plan(
    event: dict[str, Any],
    packet: dict[str, Any],
    seed_plan: dict[str, Any],
) -> dict[str, Any] | None:
    if not seed_plan:
        return None
    reason = require_str(seed_plan, "reason", "unknown")
    selected = optional_int(seed_plan.get("selectedSeedCount")) or 0
    fallback = optional_int(seed_plan.get("fallbackOwnerSeedCount")) or 0
    query_present = bool(seed_plan.get("queryPresent"))
    query_seed_present = bool(seed_plan.get("querySeedPresent"))
    risk = _seed_plan_risk(packet, seed_plan)
    if query_seed_present and selected > 0 and not risk:
        return None
    command_id = require_str(event, "commandId", require_str(event, "eventId", "command"))
    packet_seed_ids = [str(item) for item in list_value(seed_plan.get("seedIds"))]
    owner_count = optional_int(seed_plan.get("candidateOwnerCount")) or 0
    candidate_count = optional_int(seed_plan.get("candidateCount")) or 0
    if selected == 0:
        expected_change = "non-empty-seed-frontier"
        recommended_action = (
            "Inspect graph-turbo seed extraction before rank calibration; no seed "
            "ids were selected."
        )
        confidence = 0.9
    elif "flat-query" in risk:
        expected_change = "split-query-pack"
        recommended_action = (
            "Split flat seed queries into cohesive query-pack clauses before "
            "graph-turbo rank calibration."
        )
        confidence = 0.8
    elif "owner-drift" in risk:
        expected_change = "narrow-owner-scope"
        recommended_action = (
            "Narrow owner scope before graph-turbo ranking when seed candidates "
            "span too many owners."
        )
        confidence = 0.75
    else:
        expected_change = "query-seed-present"
        recommended_action = (
            "Propagate the agent query into graph-turbo seedPlan before using "
            "owner fallback as calibration evidence."
        )
        confidence = 0.65 if fallback else 0.75
    return {
        "id": f"gt.seed-plan.{command_id}",
        "kind": "seed-plan-quality",
        "confidence": confidence,
        "reason": (
            "Graph-turbo seed phase selected "
            f"{selected} seed(s) with reason={reason}, "
            f"queryPresent={query_present}, querySeedPresent={query_seed_present}, "
            f"fallbackOwnerSeedCount={fallback}, "
            f"candidateOwnerCount={owner_count}, candidateCount={candidate_count}, "
            f"risk={','.join(risk) or 'none'}."
        ),
        "evidenceRefs": [require_str(event, "eventId", command_id)],
        "packetNodeIds": packet_seed_ids,
        "expectedChange": expected_change,
        "recommendedAction": recommended_action,
    }


def _seed_plan_risk(
    packet: dict[str, Any],
    seed_plan: dict[str, Any],
) -> list[str]:
    query_term_count = len(list_value(packet.get("queryTerms")))
    owner_count = optional_int(seed_plan.get("candidateOwnerCount")) or 0
    fallback = optional_int(seed_plan.get("fallbackOwnerSeedCount")) or 0
    selected = optional_int(seed_plan.get("selectedSeedCount")) or 0
    risk = []
    if selected == 0:
        risk.append("empty-seed-frontier")
    if fallback:
        risk.append("fallback-owner")
    if query_term_count >= 6:
        risk.append("flat-query")
    if query_term_count >= 4 and owner_count >= 4:
        risk.append("owner-drift")
    return risk


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
