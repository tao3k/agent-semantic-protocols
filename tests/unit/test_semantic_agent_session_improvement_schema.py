"""Validate semantic agent session improvement report schema."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = json.loads(
    (
        _ROOT
        / "schemas"
        / "semantic-agent-session-improvement-report.v1.schema.json"
    ).read_text()
)


def test_agent_session_improvement_report_accepts_metrics_and_points() -> None:
    improvement = {
        "schemaId": (
            "agent.semantic-protocols.semantic-agent-session-improvement-report"
        ),
        "schemaVersion": "1",
        "sessionId": "session-1",
        "scenarioId": "rust.tokio-agent-observability",
        "sourceQualityReportPath": "reports/quality-report.json",
        "sourceGraphTurboFeedbackPath": "reports/graph-turbo-feedback.json",
        "metrics": _metrics(),
        "improvementPoints": [_repeated_command_point()],
    }

    Draft202012Validator(_SCHEMA).validate(improvement)


def _metrics() -> dict[str, object]:
    return {
        "commandCount": 2,
        "aspCommands": 2,
        "searchCommands": 2,
        "queryCommands": 0,
        "directReadRiskCommands": 0,
        "repeatedCommands": 1,
        "deniedCommands": 0,
        "stdoutBytes": 80,
        "stderrBytes": 0,
        "totalRounds": 2,
        "findingCount": 1,
        "graphTurboCandidateCount": 1,
        "roundStatusCounts": {"warning": 2},
        "commandKindCounts": {"search": 2},
        "qualitySignalCounts": {
            "command-recorded": 2,
            "repeated-command": 2,
        },
        "answer": {
            "present": True,
            "groundingStatus": "grounded",
            "afterLastToolUse": True,
        },
    }


def _repeated_command_point() -> dict[str, object]:
    return {
        "id": "improve.command.repeated",
        "category": "command-efficiency",
        "severity": "warning",
        "title": "Repeated command argv were recorded in the session.",
        "observed": {"metric": "repeatedCommands", "value": 1},
        "target": {"metric": "repeatedCommands", "value": 0},
        "evidenceRefs": ["round-command-call_1", "round-command-call_2"],
        "recommendedAction": "Merge repeated searches into query-set.",
        "expectedImpact": "Reduce repeated command rounds.",
        "sourceFindingIds": ["command.repeated"],
    }
