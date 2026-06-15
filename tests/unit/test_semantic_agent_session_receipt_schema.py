"""Validate semantic agent session receipt and graph feedback schemas."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_RECEIPT_SCHEMA = json.loads(
    (_ROOT / "schemas" / "semantic-agent-session-receipt.v1.schema.json").read_text()
)
_FEEDBACK_SCHEMA = json.loads(
    (
        _ROOT
        / "schemas"
        / "semantic-agent-session-graph-turbo-feedback.v1.schema.json"
    ).read_text()
)
_QUALITY_SCHEMA = json.loads(
    (
        _ROOT
        / "schemas"
        / "semantic-agent-session-quality-report.v1.schema.json"
    ).read_text()
)


def test_agent_session_receipt_accepts_minimal_grounded_session() -> None:
    Draft202012Validator(_RECEIPT_SCHEMA).validate(_receipt())


def test_agent_session_receipt_rejects_raw_message_stream() -> None:
    receipt = _receipt()
    receipt["messages"] = [{"type": "AssistantMessage", "content": []}]

    errors = list(Draft202012Validator(_RECEIPT_SCHEMA).iter_errors(receipt))

    assert errors


def test_graph_turbo_feedback_accepts_candidate_packet() -> None:
    feedback = {
        "schemaId": (
            "agent.semantic-protocols.semantic-agent-session-graph-turbo-feedback"
        ),
        "schemaVersion": "1",
        "sessionId": "session-1",
        "scenarioId": "rust.tokio-agent-observability",
        "sourceReceiptPath": "receipts/agent-session-receipt.json",
        "candidates": [
            {
                "id": "gt.command.repeated",
                "kind": "repeated-query-group",
                "confidence": 0.5,
                "reason": "Repeated search terms can become a query set.",
                "evidenceRefs": ["command-call_1", "command-call_2"],
                "recommendedAction": "Promote repeated searches into query-set guidance.",
            }
        ],
    }

    Draft202012Validator(_FEEDBACK_SCHEMA).validate(feedback)


def test_agent_session_quality_report_accepts_findings() -> None:
    quality = {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-quality-report",
        "schemaVersion": "1",
        "sessionId": "session-1",
        "scenarioId": "rust.tokio-agent-observability",
        "summary": {
            "commandCount": 2,
            "searchCommands": 2,
            "repeatedCommands": 1,
            "stdoutBytes": 40,
            "stderrBytes": 0,
            "elapsedMs": 0,
        },
        "answer": {
            "present": True,
            "afterLastToolUse": True,
            "textBytes": 40,
            "textLineCount": 1,
            "groundingStatus": "grounded",
        },
        "findings": [
            {
                "id": "command.repeated",
                "kind": "command-efficiency",
                "severity": "warning",
                "message": "Repeated command argv were recorded in the session.",
                "recommendedAction": "Merge repeated searches into query-set.",
                "graphTurboFeedback": "Promote repeated query groups.",
            }
        ],
        "turnSummary": {
            "totalTurns": 2,
            "phaseCounts": {"answer": 1, "command-result": 1},
            "qualitySignalCounts": {
                "answer-grounded": 1,
                "command-recorded": 1,
                "repeated-command": 1,
            },
            "findingLinkedTurns": 1,
        },
        "turnDetails": [
            {
                "id": "turn-command-call_1",
                "ordinal": 0,
                "phase": "command-result",
                "commandId": "command-call_1",
                "commandKind": "search",
                "argv": ["asp", "rust", "search", "prime", "--view", "seeds", "."],
                "metrics": {"stdoutBytes": 40, "stderrBytes": 0, "elapsedMs": 0},
                "qualitySignals": ["command-recorded", "repeated-command"],
                "findingIds": ["command.repeated"],
            },
            {
                "id": "turn-answer",
                "ordinal": 1,
                "phase": "answer",
                "qualitySignals": ["answer-grounded"],
            },
        ],
        "roundSummary": {
            "totalRounds": 1,
            "commandKindCounts": {"search": 1},
            "qualitySignalCounts": {
                "command-recorded": 1,
                "repeated-command": 1,
            },
            "findingLinkedRounds": 1,
            "deniedRounds": 0,
            "riskRounds": 0,
            "repeatedRounds": 1,
        },
        "roundDetails": [
            {
                "id": "round-command-call_1",
                "ordinal": 0,
                "commandId": "command-call_1",
                "commandKind": "search",
                "argv": ["asp", "rust", "search", "prime", "--view", "seeds", "."],
                "metrics": {"stdoutBytes": 40, "stderrBytes": 0, "elapsedMs": 0},
                "qualitySignals": ["command-recorded", "repeated-command"],
                "findingIds": ["command.repeated"],
                "resultStatus": "warning",
            }
        ],
    }

    Draft202012Validator(_QUALITY_SCHEMA).validate(quality)


def _receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-receipt",
        "schemaVersion": "1",
        "sessionId": "session-1",
        "scenarioId": "rust.tokio-agent-observability",
        "language": "rust",
        "project": {"name": "tokio", "source": "registry"},
        "intent": "Explain Tokio IO readiness.",
        "agent": "claude-sdk",
        "model": "sonnet",
        "startedAtUtc": "2026-06-14T12:00:00Z",
        "finishedAtUtc": "2026-06-14T12:00:02Z",
        "editBoundary": "before-edit",
        "artifactRoot": ".cache/agent-session/session-1",
        "summary": {
            "turns": 4,
            "assistantVisibleMessages": 1,
            "toolRequests": 1,
            "toolResults": 1,
            "commandCount": 1,
            "aspCommands": 1,
            "searchCommands": 1,
            "searchPrimeCommands": 1,
            "queryCommands": 0,
            "checkCommands": 0,
            "guideCommands": 0,
            "deniedCommands": 0,
            "repeatedCommands": 0,
            "directReadRiskCommands": 0,
            "stdoutBytes": 20,
            "stderrBytes": 0,
            "elapsedMs": 0,
        },
        "answer": {
            "present": True,
            "afterLastToolUse": True,
            "textBytes": 40,
            "textLineCount": 1,
            "messageEventId": "session-0005-answer-final",
            "evidenceRefs": ["command-call_1"],
            "groundingStatus": "grounded",
            "preview": "Tokio IO readiness is grounded in search output.",
        },
        "commands": [
            {
                "id": "command-call_1",
                "kind": "search",
                "argv": ["asp", "rust", "search", "prime", "--view", "seeds", "."],
                "metrics": {"elapsedMs": 0, "stdoutBytes": 20, "stderrBytes": 0},
            }
        ],
        "qualityFindings": [],
    }
