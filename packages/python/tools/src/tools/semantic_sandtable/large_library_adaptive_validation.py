"""Validate adaptive graph-turbo policy runs against question-level receipts."""

from __future__ import annotations

from typing import Any

from .utils import dict_value, list_value, optional_int, require_str

_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-adaptive-validation-report"


def build_large_library_adaptive_validation_report(
    policy: dict[str, Any],
    question_plan: dict[str, Any],
) -> dict[str, Any]:
    planned_runs = [
        item
        for item in list_value(dict_value(policy.get("validationPlan")).get("runs"))
        if isinstance(item, dict)
    ]
    questions = [
        item
        for item in list_value(question_plan.get("questions"))
        if isinstance(item, dict)
    ]
    observed = _observed_question_index(questions)
    run_results = [
        _run_result(run, observed)
        for run in planned_runs
    ]
    summary = _summary(run_results)
    return {
        "schemaId": _SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-adaptive-validation-report",
        "targetGraphPhase": "query-first-stage",
        "status": _coverage_status(summary),
        "sourcePolicy": _source_policy(policy),
        "sourceQuestionPlan": _source_question_plan(question_plan),
        "summary": summary,
        "variantSummaries": _variant_summaries(run_results),
        "runResults": run_results,
        "missingRuns": [
            result["plannedRun"]
            for result in run_results
            if result["coverage"] == "missing"
        ],
        "promotionReadiness": _promotion_readiness(summary),
    }


def _observed_question_index(
    questions: list[dict[str, Any]],
) -> dict[str, tuple[dict[str, Any], str]]:
    observed: dict[str, tuple[dict[str, Any], str]] = {}
    for question in questions:
        source = dict_value(question.get("sourceSession"))
        session_id = require_str(source, "sessionId", "")
        if session_id:
            observed.setdefault(session_id, (question, "source-session-id"))
        question_id = require_str(question, "id", "")
        if question_id:
            observed.setdefault(question_id, (question, "question-id"))
    return observed


def _run_result(
    run: dict[str, Any],
    observed: dict[str, tuple[dict[str, Any], str]],
) -> dict[str, Any]:
    run_id = require_str(run, "runId", "unknown")
    planned = {
        "runId": run_id,
        "scenarioId": require_str(run, "scenarioId", "unknown"),
        "scenarioPath": require_str(run, "scenarioPath", "unknown"),
        "questionId": require_str(run, "questionId", "unknown"),
        "ablationVariant": require_str(run, "ablationVariant", "unknown"),
    }
    match = _match_observed_question(run_id, planned["questionId"], observed)
    if match is None:
        return {
            "coverage": "missing",
            "plannedRun": planned,
            "matchingStrategy": "none",
        }
    question, strategy = match
    analyzer = dict_value(question.get("analyzerJudgment"))
    final_answer = dict_value(question.get("finalAnswer"))
    natural_language = dict_value(question.get("naturalLanguageSignals"))
    human_review = dict_value(question.get("humanReview"))
    metrics = dict_value(question.get("analysisMetrics"))
    return {
        "coverage": "observed",
        "plannedRun": planned,
        "matchingStrategy": strategy,
        "observedQuestion": {
            "id": require_str(question, "id", "unknown"),
            "sourceSession": dict_value(question.get("sourceSession")),
            "sourceArtifacts": dict_value(question.get("sourceArtifacts")),
            "language": require_str(question, "language", "unknown"),
            "project": dict_value(question.get("project")),
        },
        "quality": {
            "analyzerStatus": require_str(analyzer, "status", "unknown"),
            "answerGrounding": require_str(analyzer, "answerGrounding", "unknown"),
            "findingIds": [str(item) for item in list_value(analyzer.get("findingIds"))],
            "graphTurboCandidateKinds": [
                str(item)
                for item in list_value(analyzer.get("graphTurboCandidateKinds"))
            ],
            "finalAnswerPresent": bool(final_answer.get("present")),
            "humanReviewStatus": require_str(human_review, "status", "unknown"),
            "revealedSignals": [
                str(item)
                for item in list_value(natural_language.get("revealedSignals"))
            ],
        },
        "roundMetrics": {
            "totalTurns": optional_int(metrics.get("totalTurns")) or 0,
            "totalRounds": optional_int(metrics.get("totalRounds")) or 0,
            "findingLinkedTurns": optional_int(metrics.get("findingLinkedTurns")) or 0,
            "findingLinkedRounds": optional_int(metrics.get("findingLinkedRounds"))
            or 0,
            "deniedRounds": optional_int(metrics.get("deniedRounds")) or 0,
            "riskRounds": optional_int(metrics.get("riskRounds")) or 0,
            "repeatedRounds": optional_int(metrics.get("repeatedRounds")) or 0,
        },
        "planItemCount": len(list_value(question.get("improvementPlan"))),
    }


def _match_observed_question(
    run_id: str,
    question_id: str,
    observed: dict[str, tuple[dict[str, Any], str]],
) -> tuple[dict[str, Any], str] | None:
    candidates = (
        (run_id, "source-session-id=runId"),
        (f"question.{_safe_id(run_id)}", "question-id=safe-runId"),
        (question_id, "question-id=planned-questionId"),
    )
    for key, strategy in candidates:
        match = observed.get(key)
        if match is not None:
            question, _ = match
            return question, strategy
    return None


def _summary(run_results: list[dict[str, Any]]) -> dict[str, Any]:
    observed_runs = [
        result for result in run_results if result.get("coverage") == "observed"
    ]
    missing_runs = len(run_results) - len(observed_runs)
    return {
        "plannedRunCount": len(run_results),
        "observedRunCount": len(observed_runs),
        "missingRunCount": missing_runs,
        "coverageRatio": _ratio(len(observed_runs), len(run_results)),
        "plannedVariantCounts": _planned_variant_counts(run_results),
        "observedVariantCounts": _planned_variant_counts(observed_runs),
        "missingVariantCounts": _planned_variant_counts(
            [
                result
                for result in run_results
                if result.get("coverage") == "missing"
            ]
        ),
        "analyzerStatusCounts": _quality_counts(observed_runs, "analyzerStatus"),
        "answerGroundingCounts": _quality_counts(observed_runs, "answerGrounding"),
        "humanReviewStatusCounts": _quality_counts(observed_runs, "humanReviewStatus"),
        "roundTotals": _round_totals(observed_runs),
        "graphTurboCandidateKindCounts": _quality_list_counts(
            observed_runs,
            "graphTurboCandidateKinds",
        ),
        "revealedSignalCounts": _quality_list_counts(observed_runs, "revealedSignals"),
    }


def _source_policy(policy: dict[str, Any]) -> dict[str, Any]:
    validation_plan = dict_value(policy.get("validationPlan"))
    default_policy = dict_value(policy.get("defaultPolicy"))
    return {
        "schemaId": policy.get("schemaId"),
        "packetKind": policy.get("packetKind"),
        "status": policy.get("status"),
        "defaultVariant": default_policy.get("ablationVariant"),
        "plannedRunCount": validation_plan.get("runCount"),
    }


def _source_question_plan(question_plan: dict[str, Any]) -> dict[str, Any]:
    rollup = dict_value(question_plan.get("rollup"))
    return {
        "schemaId": question_plan.get("schemaId"),
        "scenarioId": question_plan.get("scenarioId"),
        "questionCount": rollup.get("questionCount"),
        "pendingHumanReviews": rollup.get("pendingHumanReviews"),
        "sourceQuestionPlanPaths": list_value(
            question_plan.get("sourceQuestionPlanPaths")
        ),
    }


def _coverage_status(summary: dict[str, Any]) -> str:
    if summary["plannedRunCount"] == 0:
        return "empty"
    if summary["missingRunCount"] == 0:
        return "complete"
    if summary["observedRunCount"] > 0:
        return "partial"
    return "missing"


def _variant_summaries(
    run_results: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    variants = sorted(
        {
            require_str(dict_value(result.get("plannedRun")), "ablationVariant", "")
            for result in run_results
        }
        - {""}
    )
    summaries = []
    for variant in variants:
        variant_results = [
            result
            for result in run_results
            if require_str(
                dict_value(result.get("plannedRun")),
                "ablationVariant",
                "",
            )
            == variant
        ]
        observed = [
            result for result in variant_results if result.get("coverage") == "observed"
        ]
        rounds = [
            optional_int(dict_value(result.get("roundMetrics")).get("totalRounds"))
            or 0
            for result in observed
        ]
        summaries.append(
            {
                "ablationVariant": variant,
                "plannedRunCount": len(variant_results),
                "observedRunCount": len(observed),
                "missingRunCount": len(variant_results) - len(observed),
                "averageRounds": _average(rounds),
                "analyzerStatusCounts": _quality_counts(
                    observed,
                    "analyzerStatus",
                ),
                "roundTotals": _round_totals(observed),
            }
        )
    return summaries


def _promotion_readiness(summary: dict[str, Any]) -> dict[str, Any]:
    blocking_reasons = []
    if summary["missingRunCount"]:
        blocking_reasons.append("missing-validation-runs")
    if summary["analyzerStatusCounts"].get("fail", 0):
        blocking_reasons.append("failing-observed-runs")
    if summary["analyzerStatusCounts"].get("review", 0):
        blocking_reasons.append("review-required-observed-runs")
    if summary["humanReviewStatusCounts"].get("pending", 0):
        blocking_reasons.append("pending-human-review")
    next_actions = []
    if "missing-validation-runs" in blocking_reasons:
        next_actions.append(
            "Run the missing validationPlan entries as live agent sessions using runId as sessionId."
        )
    if "review-required-observed-runs" in blocking_reasons:
        next_actions.append(
            "Inspect analyzer findings and final-answer previews for observed review runs."
        )
    if "pending-human-review" in blocking_reasons:
        next_actions.append(
            "Mark each question-level human review accepted, rejected, or needing rerun."
        )
    if not next_actions:
        next_actions.append("Eligible for guarded runtime promotion review.")
    return {
        "status": "blocked" if blocking_reasons else "eligible",
        "blockingReasons": blocking_reasons,
        "nextActions": next_actions,
    }


def _planned_variant_counts(run_results: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for result in run_results:
        planned = dict_value(result.get("plannedRun"))
        variant = require_str(planned, "ablationVariant", "unknown")
        counts[variant] = counts.get(variant, 0) + 1
    return dict(sorted(counts.items()))


def _quality_counts(run_results: list[dict[str, Any]], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for result in run_results:
        quality = dict_value(result.get("quality"))
        value = require_str(quality, key, "unknown")
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def _quality_list_counts(
    run_results: list[dict[str, Any]],
    key: str,
) -> dict[str, int]:
    counts: dict[str, int] = {}
    for result in run_results:
        quality = dict_value(result.get("quality"))
        for value in list_value(quality.get(key)):
            item = str(value)
            counts[item] = counts.get(item, 0) + 1
    return dict(sorted(counts.items()))


def _round_totals(run_results: list[dict[str, Any]]) -> dict[str, int]:
    totals = {
        "totalTurns": 0,
        "totalRounds": 0,
        "findingLinkedTurns": 0,
        "findingLinkedRounds": 0,
        "deniedRounds": 0,
        "riskRounds": 0,
        "repeatedRounds": 0,
    }
    for result in run_results:
        metrics = dict_value(result.get("roundMetrics"))
        for key in totals:
            totals[key] += optional_int(metrics.get(key)) or 0
    return totals


def _ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 0.0
    return round(numerator / denominator, 4)


def _average(values: list[int]) -> float:
    if not values:
        return 0.0
    return round(sum(values) / len(values), 4)


def _safe_id(value: str) -> str:
    return "".join(
        character if character.isalnum() or character in {".", "-", "_"} else "-"
        for character in value
    ).strip("-") or "session"
