"""Build graph-turbo adaptive policy candidates from large-library analysis."""

from __future__ import annotations

from typing import Any

from .utils import dict_value, list_value, require_str

_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-adaptive-query-policy"
_VARIANT_POLICIES = {
    "no-query-seed-prior": {"seedPrior": False},
    "no-package-cohesion": {"packageCohesion": False},
    "no-query-clause-coverage": {"queryClauseCoverage": False},
}


def build_large_library_adaptive_policy(
    analysis: dict[str, Any],
) -> dict[str, Any]:
    recommendations = dict_value(analysis.get("variantRecommendations"))
    overall_winner = dict_value(recommendations.get("overallWinner"))
    bucket_winners = [
        item
        for item in list_value(recommendations.get("bucketWinners"))
        if isinstance(item, dict)
    ]
    status = _policy_status(analysis, recommendations, overall_winner)
    default_variant = require_str(overall_winner, "ablationVariant", "")
    bucket_policies = _bucket_policy_entries(bucket_winners)
    return {
        "schemaId": _SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-adaptive-query-policy",
        "targetGraphPhase": "query-first-stage",
        "status": status,
        "policyMode": "adaptive-by-language-package-depth",
        "rankingMetric": recommendations.get("rankingMetric"),
        "sourceAnalysis": _source_analysis(analysis),
        "defaultPolicy": _policy_entry(default_variant, overall_winner),
        "bucketPolicies": bucket_policies,
        "validationPlan": _validation_plan(bucket_policies),
        "runtimeBridge": {
            "envVar": "ASP_GRAPH_TURBO_ABLATION_VARIANT",
            "supportedVariants": sorted(_VARIANT_POLICIES),
        },
        "guardrails": _guardrails(),
    }


def _policy_status(
    analysis: dict[str, Any],
    recommendations: dict[str, Any],
    overall_winner: dict[str, Any],
) -> str:
    summary = dict_value(analysis.get("summary"))
    if (
        summary.get("status") == "analyzed"
        and recommendations.get("status") == "ready"
        and _policy_for_variant(require_str(overall_winner, "ablationVariant", ""))
    ):
        return "ready"
    return "collecting"


def _source_analysis(analysis: dict[str, Any]) -> dict[str, Any]:
    summary = dict_value(analysis.get("summary"))
    source = dict_value(analysis.get("sourceReportChain"))
    return {
        "schemaId": analysis.get("schemaId"),
        "packetKind": analysis.get("packetKind"),
        "status": summary.get("status"),
        "expectedVariantRunCount": summary.get("expectedVariantRunCount"),
        "observedVariantRunCount": summary.get("observedVariantRunCount"),
        "findingCount": summary.get("findingCount"),
        "targetGraphPhase": source.get("targetGraphPhase"),
    }


def _policy_entry(variant: str, metrics: dict[str, Any]) -> dict[str, Any] | None:
    policy = _policy_for_variant(variant)
    if policy is None:
        return None
    return {
        "ablationVariant": variant,
        "queryAdjustmentPolicy": policy,
        "averageAnswerQuality": metrics.get("averageAnswerQuality"),
        "averageElapsedMs": metrics.get("averageElapsedMs"),
        "averageAspCommandCount": metrics.get("averageAspCommandCount"),
        "averageStdoutBytes": metrics.get("averageStdoutBytes"),
        "resultCount": metrics.get("resultCount"),
    }


def _bucket_policy_entry(item: dict[str, Any]) -> dict[str, Any] | None:
    entry = _policy_entry(require_str(item, "ablationVariant", ""), item)
    if entry is None:
        return None
    entry.update(
        {
            "language": item.get("language"),
            "package": item.get("package"),
            "depthBucket": item.get("depthBucket"),
            "candidateCount": item.get("candidateCount"),
            "evidence": dict_value(item.get("evidence")),
        }
    )
    return entry


def _bucket_policy_entries(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    entries = []
    for item in items:
        entry = _bucket_policy_entry(item)
        if entry is not None:
            entries.append(entry)
    return entries


def _validation_plan(bucket_policies: list[dict[str, Any]]) -> dict[str, Any]:
    runs = [
        run
        for policy in bucket_policies
        for run in _validation_runs_for_policy(policy)
    ]
    return {
        "targetGranularity": "per-question-live-agent-receipt",
        "runCount": len(runs),
        "runs": runs,
    }


def _validation_runs_for_policy(policy: dict[str, Any]) -> list[dict[str, Any]]:
    evidence = dict_value(policy.get("evidence"))
    scenario_ids = list_value(evidence.get("scenarioIds"))
    scenario_paths = list_value(evidence.get("scenarioPaths"))
    question_ids = list_value(evidence.get("questionIds"))
    scenario_id = _first_string(scenario_ids, "unknown")
    scenario_path = _first_string(scenario_paths, "unknown")
    variant = require_str(policy, "ablationVariant", "unknown")
    return [
        {
            "runId": (
                f"{scenario_id}:{question_id}:"
                f"{variant}:per-question-live-agent"
            ),
            "scenarioId": scenario_id,
            "scenarioPath": scenario_path,
            "questionId": str(question_id),
            "ablationVariant": variant,
            "env": {
                "ASP_GRAPH_TURBO_ABLATION_VARIANT": variant,
            },
            "expectedReceiptGranularity": "per-question-live-agent",
        }
        for question_id in question_ids
        if isinstance(question_id, str) and question_id
    ]


def _first_string(values: list[object], default: str) -> str:
    for value in values:
        if isinstance(value, str) and value:
            return value
    return default


def _policy_for_variant(variant: str) -> dict[str, bool] | None:
    policy = _VARIANT_POLICIES.get(variant)
    return dict(policy) if policy is not None else None


def _guardrails() -> dict[str, Any]:
    return {
        "minAverageAnswerQuality": 0.75,
        "requiresAnalyzedSource": True,
        "volatileMetricsExcludedFromEquivalence": ["elapsedMs"],
        "bucketGranularity": "scenario-receipt",
        "nextValidationGranularity": "per-question-live-agent-receipt",
        "promotion": "requires fresh sandtable receipt coverage before runtime default change",
    }
