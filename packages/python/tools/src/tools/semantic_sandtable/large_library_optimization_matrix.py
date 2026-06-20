"""Graph-turbo optimization matrix for large-library report chains."""

from __future__ import annotations

from typing import Any

QUERY_FIRST_STAGE_ABLATION_VARIANTS = (
    "no-query-seed-prior",
    "no-package-cohesion",
    "no-query-clause-coverage",
    "no-local-evidence",
)
OPTIMIZATION_BATCH_AGGREGATION_AXES = (
    "language",
    "package",
    "depthBucket",
    "ablationVariant",
)
REQUIRED_RECEIPT_METRICS = (
    "aspCommandCount",
    "searchCommandCount",
    "queryCommandCount",
    "repeatedCommandCount",
    "commandsToFirstUsefulLocator",
    "frontierFollowRate",
    "rawReadFallbackCount",
    "duplicateSelectorCount",
    "sameOwnerScanCount",
    "elapsedMs",
    "stdoutBytes",
    "stderrBytes",
)
REQUIRED_ANSWER_METRICS = (
    "finalAnswerStatus",
    "answerQualityJudgment",
    "missingEvidenceCount",
    "wrongOwnerCount",
)


def optimization_matrix(scenarios: list[dict[str, Any]]) -> list[dict[str, Any]]:
    runs: list[dict[str, Any]] = []
    for scenario in scenarios:
        for question in scenario["deepQuestions"]:
            runs.append(_optimization_run(scenario, question))
    return sorted(
        runs,
        key=lambda run: (
            str(run["language"]),
            str(run["depthBucket"]),
            str(run["package"]),
            str(run["questionId"]),
        ),
    )


def optimization_batch(matrix: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "targetGraphPhase": "query-first-stage",
        "nextStage": "collect-receipts",
        "readyToCollectReceipts": bool(matrix),
        "runCount": len(matrix),
        "ablationVariantCount": len(QUERY_FIRST_STAGE_ABLATION_VARIANTS),
        "variantRunCount": len(matrix) * len(QUERY_FIRST_STAGE_ABLATION_VARIANTS),
        "ablationVariants": list(QUERY_FIRST_STAGE_ABLATION_VARIANTS),
        "aggregationAxes": list(OPTIMIZATION_BATCH_AGGREGATION_AXES),
        "requiredReceiptMetrics": list(REQUIRED_RECEIPT_METRICS),
        "requiredAnswerMetrics": list(REQUIRED_ANSWER_METRICS),
    }


def _optimization_run(
    scenario: dict[str, Any], question: dict[str, Any]
) -> dict[str, Any]:
    required_signals = [
        name
        for name in (
            "requiresQuerySet",
            "requiresGraphSignals",
            "requiresHookEvents",
            "requiresComplexPipeFlow",
            "requiresTokenCost",
        )
        if question.get(name) is True
    ]
    return {
        "runId": (
            f"{scenario['language']}:{scenario['package']}:"
            f"{question['id']}:query-first-stage"
        ),
        "language": scenario["language"],
        "scenarioId": scenario["scenarioId"],
        "scenarioPath": scenario["path"],
        "package": scenario["package"],
        "repository": scenario["repository"],
        "questionId": question["id"],
        "depthBucket": question["depthBucket"],
        "fixtureTier": scenario["fixtureTier"],
        "live": scenario["live"],
        "promptOnly": scenario["promptOnly"],
        "maxAspCommands": question["maxAspCommands"],
        "requiredSignals": required_signals,
        "requiredStages": question["requiredStages"],
        "targetGraphPhase": "query-first-stage",
        "ablationVariants": list(QUERY_FIRST_STAGE_ABLATION_VARIANTS),
    }
