"""Adaptive graph-turbo validation report tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_adaptive_validation import (
    build_large_library_adaptive_validation_report,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_adaptive_validation_matches_question_plan_by_run_id() -> None:
    report = build_large_library_adaptive_validation_report(
        _policy(),
        _question_plan(),
    )

    _validate_schema(report)
    assert report["status"] == "partial"
    assert report["summary"]["plannedRunCount"] == 2
    assert report["summary"]["observedRunCount"] == 1
    assert report["summary"]["missingRunCount"] == 1
    assert report["summary"]["coverageRatio"] == 0.5
    assert report["summary"]["observedVariantCounts"] == {
        "no-query-seed-prior": 1
    }
    assert report["summary"]["missingVariantCounts"] == {
        "no-package-cohesion": 1
    }
    assert report["summary"]["analyzerStatusCounts"] == {"pass": 1}
    assert report["summary"]["roundTotals"]["totalRounds"] == 2
    observed = report["runResults"][0]
    assert observed["coverage"] == "observed"
    assert observed["matchingStrategy"] == "source-session-id=runId"
    assert observed["quality"]["answerGrounding"] == "grounded"
    assert observed["roundMetrics"]["totalTurns"] == 5
    assert report["missingRuns"][0]["runId"] == "run-b"
    assert report["promotionReadiness"]["status"] == "blocked"
    assert report["promotionReadiness"]["blockingReasons"] == [
        "missing-validation-runs",
        "pending-human-review",
    ]


def test_adaptive_validation_cli_writes_report(tmp_path: Path) -> None:
    policy_path = tmp_path / "policy.json"
    question_plan_path = tmp_path / "question-plan.json"
    output_path = tmp_path / "validation-report.json"
    policy_path.write_text(json.dumps(_policy()), encoding="utf-8")
    question_plan_path.write_text(json.dumps(_question_plan()), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-adaptive-validation",
                str(policy_path),
                "--question-plan",
                str(question_plan_path),
                "--output",
                str(output_path),
            ]
        )
        == 0
    )

    report = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema(report)
    assert report["summary"]["plannedRunCount"] == 2


def _policy() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-adaptive-query-policy",
        "packetKind": "graph-turbo-adaptive-query-policy",
        "status": "ready",
        "defaultPolicy": {"ablationVariant": "no-package-cohesion"},
        "validationPlan": {
            "runCount": 2,
            "runs": [
                {
                    "runId": "run-a",
                    "scenarioId": "rust.bytes-intent-matrix",
                    "scenarioPath": "sandtables/rust/bytes-intent-matrix.json",
                    "questionId": "bytes-bufmut-unsafely-advance",
                    "ablationVariant": "no-query-seed-prior",
                    "env": {
                        "ASP_GRAPH_TURBO_ABLATION_VARIANT": "no-query-seed-prior"
                    },
                    "expectedReceiptGranularity": "per-question-live-agent",
                },
                {
                    "runId": "run-b",
                    "scenarioId": "rust.bytes-intent-matrix",
                    "scenarioPath": "sandtables/rust/bytes-intent-matrix.json",
                    "questionId": "bytes-split-freeze-sharing",
                    "ablationVariant": "no-package-cohesion",
                    "env": {
                        "ASP_GRAPH_TURBO_ABLATION_VARIANT": "no-package-cohesion"
                    },
                    "expectedReceiptGranularity": "per-question-live-agent",
                },
            ],
        },
    }


def _question_plan() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-question-plan",
        "schemaVersion": "1",
        "sessionId": "question-plan-aggregate",
        "scenarioId": "agent-session.question-plan-aggregate",
        "sourceReceiptPath": "multiple",
        "sourceQualityReportPath": "multiple",
        "sourceGraphTurboFeedbackPath": "multiple",
        "sourceImprovementReportPath": "multiple",
        "sourceQuestionPlanPaths": ["run-a/reports/question-improvement-plan.json"],
        "questions": [_observed_question()],
        "rollup": _question_plan_rollup(),
    }


def _observed_question() -> dict[str, object]:
    return {
        "id": "question.run-a",
        "sourceSession": {
            "sessionId": "run-a",
            "scenarioId": "rust.bytes-intent-matrix",
        },
        "sourceArtifacts": {
            "receiptPath": "run-a/receipts/agent-session-receipt.json",
            "qualityReportPath": "run-a/reports/quality-report.json",
            "graphTurboFeedbackPath": "run-a/reports/graph-turbo-feedback.json",
            "improvementReportPath": "run-a/reports/improvement-report.json",
        },
        "question": "How should BufMut unsafely advance be located before editing?",
        "language": "rust",
        "project": {"name": "bytes"},
        "analysisMetrics": {
            "totalTurns": 5,
            "totalRounds": 2,
            "findingLinkedTurns": 0,
            "findingLinkedRounds": 0,
            "deniedRounds": 0,
            "riskRounds": 0,
            "repeatedRounds": 0,
        },
        "finalAnswer": {
            "present": True,
            "groundingStatus": "grounded",
            "afterLastToolUse": True,
            "textBytes": 64,
            "textLineCount": 1,
            "preview": "Use the located bytes owner and exact selector evidence.",
            "evidenceRefs": [],
        },
        "naturalLanguageSignals": {
            "assistantVisibleMessageCount": 1,
            "visibleMessagePreviews": ["I found the selector evidence."],
            "finalAnswerPreview": "Use the located bytes owner and exact selector evidence.",
            "revealedSignals": ["mentions-evidence"],
        },
        "analyzerJudgment": {
            "status": "pass",
            "answerGrounding": "grounded",
            "findingIds": [],
            "graphTurboCandidateKinds": [],
            "summary": "Analyzer status=pass; findings=0; graphTurboCandidates=0.",
        },
        "humanReview": {
            "status": "pending",
            "required": True,
            "instruction": "Review the final answer.",
        },
        "improvementPlan": [],
    }


def _question_plan_rollup() -> dict[str, object]:
    return {
        "questionCount": 1,
        "pendingHumanReviews": 1,
        "analyzerStatusCounts": {"pass": 1},
        "answerGroundingCounts": {"grounded": 1},
        "languageCounts": {"rust": 1},
        "projectCounts": {"bytes": 1},
        "revealedSignalCounts": {"mentions-evidence": 1},
        "findingCounts": {},
        "graphTurboCandidateKindCounts": {},
        "planItemCount": 0,
        "planItemIdCounts": {},
        "planItemCategoryCounts": {},
        "planItemSeverityCounts": {},
        "planItemSourceCounts": {},
    }


def _validate_schema(
    packet: dict[str, object],
    schema_name: str = "semantic-graph-turbo-adaptive-validation-report.v1.schema.json",
) -> None:
    schema = json.loads(
        (_ROOT / "schemas" / schema_name).read_text(encoding="utf-8")
    )
    Draft202012Validator(schema).validate(packet)
