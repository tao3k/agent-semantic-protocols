"""Analyze large-library optimization batch results."""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any

from .utils import dict_value, list_value, optional_int, require_str


def build_large_library_optimization_analysis(
    report_chain: dict[str, Any],
    variant_results: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    results = [item for item in variant_results or [] if isinstance(item, dict)]
    matrix = [item for item in list_value(report_chain.get("optimizationMatrix")) if isinstance(item, dict)]
    batch = dict_value(report_chain.get("optimizationBatch"))
    expected_runs = _expected_variant_runs(matrix)
    expected_variant_runs = _expected_variant_run_count(batch, matrix)
    observed = _observed_results(matrix, results)
    collection = _collection_manifest(batch, expected_runs, observed)
    aggregations = _aggregations(observed)
    findings = _findings(expected_variant_runs, observed, aggregations)
    recommendations = _variant_recommendations(observed, aggregations)
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-large-library-optimization-analysis",
        "schemaVersion": "1",
        "packetKind": "large-library-optimization-analysis",
        "sourceReportChain": _source_report_chain(report_chain),
        "summary": _summary(expected_variant_runs, observed, findings),
        "collectionManifest": collection,
        "aggregations": aggregations,
        "variantRecommendations": recommendations,
        "improvementPlan": _improvement_plan(findings, aggregations, recommendations),
        "findings": findings,
    }


def _expected_variant_run_count(
    batch: dict[str, Any], matrix: list[dict[str, Any]]
) -> int:
    count = optional_int(batch.get("variantRunCount"))
    if count is not None:
        return count
    return sum(len(list_value(run.get("ablationVariants"))) for run in matrix)


def _expected_variant_runs(matrix: list[dict[str, Any]]) -> list[dict[str, Any]]:
    expected = []
    for run in matrix:
        for variant in list_value(run.get("ablationVariants")):
            if not isinstance(variant, str):
                continue
            expected.append(
                {
                    "variantRunId": _variant_run_id(run, variant),
                    "runId": run.get("runId"),
                    "language": run.get("language"),
                    "scenarioId": run.get("scenarioId"),
                    "scenarioPath": run.get("scenarioPath"),
                    "package": run.get("package"),
                    "questionId": run.get("questionId"),
                    "depthBucket": run.get("depthBucket"),
                    "ablationVariant": variant,
                    "targetGraphPhase": run.get("targetGraphPhase"),
                }
            )
    return sorted(
        expected,
        key=lambda item: (
            str(item["language"]),
            str(item["depthBucket"]),
            str(item["package"]),
            str(item["questionId"]),
            str(item["ablationVariant"]),
        ),
    )


def _observed_results(
    matrix: list[dict[str, Any]], results: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    runs = {str(run.get("runId")): run for run in matrix}
    observed = []
    for result in results:
        run_id = require_str(result, "runId", "")
        run = runs.get(run_id, {})
        observed.append(
            {
                "runId": run_id,
                "language": require_str(result, "language", require_str(run, "language", "unknown")),
                "package": require_str(result, "package", require_str(run, "package", "unknown")),
                "depthBucket": require_str(result, "depthBucket", require_str(run, "depthBucket", "unknown")),
                "scenarioId": require_str(run, "scenarioId", "unknown"),
                "scenarioPath": require_str(run, "scenarioPath", "unknown"),
                "questionId": require_str(run, "questionId", "unknown"),
                "ablationVariant": require_str(result, "ablationVariant", "unknown"),
                "variantRunId": _variant_run_id(result, require_str(result, "ablationVariant", "unknown")),
                "receiptMetrics": dict_value(result.get("receiptMetrics")),
                "answerMetrics": dict_value(result.get("answerMetrics")),
                "sourceReceiptPath": result.get("sourceReceiptPath"),
            }
        )
    return observed


def _collection_manifest(
    batch: dict[str, Any],
    expected_runs: list[dict[str, Any]],
    observed: list[dict[str, Any]],
) -> dict[str, Any]:
    observed_by_id = {
        str(item.get("variantRunId")): item
        for item in observed
        if item.get("variantRunId")
    }
    observed_ids = sorted(
        str(item.get("variantRunId")) for item in observed if item.get("variantRunId")
    )
    collection_runs = [
        _collection_run(expected, observed_by_id.get(str(expected["variantRunId"])))
        for expected in expected_runs
    ]
    missing = [
        item for item in collection_runs if item["collectionStatus"] == "missing-result"
    ]
    needs_variant_receipt = [
        item for item in collection_runs if item["needsVariantReceipt"] is True
    ]
    return {
        "resultPacketKind": "large-library-optimization-variant-result",
        "targetGraphPhase": batch.get("targetGraphPhase"),
        "expectedVariantRuns": expected_runs,
        "missingVariantRuns": missing,
        "collectionRuns": collection_runs,
        "runsNeedingVariantReceipt": needs_variant_receipt,
        "collectionStatus": "collected" if not needs_variant_receipt else "collecting",
        "metricSourceCounts": _metric_source_counts(collection_runs),
        "observedVariantRunIds": observed_ids,
        "requiredReceiptMetrics": list_value(batch.get("requiredReceiptMetrics")),
        "requiredAnswerMetrics": list_value(batch.get("requiredAnswerMetrics")),
    }


def _collection_run(
    expected: dict[str, Any],
    observed: dict[str, Any] | None,
) -> dict[str, Any]:
    item = dict(expected)
    if observed is None:
        item.update(
            {
                "collectionStatus": "missing-result",
                "metricSource": "missing",
                "needsVariantReceipt": True,
            }
        )
        return item
    metric_source = _metric_source(observed)
    collection_status = _collection_status(metric_source)
    item.update(
        {
            "collectionStatus": collection_status,
            "metricSource": metric_source,
            "needsVariantReceipt": collection_status != "collected",
            "answerStatus": require_str(
                dict_value(observed.get("answerMetrics")),
                "finalAnswerStatus",
                "unknown",
            ),
        }
    )
    source_path = observed.get("sourceReceiptPath")
    if isinstance(source_path, str) and source_path:
        item["sourceReceiptPath"] = source_path
    return item


def _metric_source(observed: dict[str, Any]) -> str:
    source = dict_value(observed.get("receiptMetrics")).get("metricSource")
    if isinstance(source, str) and source:
        return source
    return "variant-result-packet"


def _collection_status(metric_source: str) -> str:
    if metric_source in {"variant-sandtable-receipt", "variant-result-packet"}:
        return "collected"
    if metric_source == "source-equivalent-variant-receipt":
        return "source-equivalent"
    if metric_source == "source-sandtable-receipt":
        return "baseline-derived"
    if metric_source == "fallback":
        return "fallback-derived"
    return "unknown-source"


def _metric_source_counts(collection_runs: list[dict[str, Any]]) -> dict[str, int]:
    counts = Counter(str(item["metricSource"]) for item in collection_runs)
    return dict(sorted(counts.items()))


def _aggregations(observed: list[dict[str, Any]]) -> list[dict[str, Any]]:
    buckets: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for result in observed:
        key = (
            str(result["language"]),
            str(result["package"]),
            str(result["depthBucket"]),
            str(result["ablationVariant"]),
        )
        buckets[key].append(result)
    return [_aggregation(key, items) for key, items in sorted(buckets.items())]


def _aggregation(
    key: tuple[str, str, str, str], items: list[dict[str, Any]]
) -> dict[str, Any]:
    language, package, depth_bucket, variant = key
    return {
        "language": language,
        "package": package,
        "depthBucket": depth_bucket,
        "ablationVariant": variant,
        "resultCount": len(items),
        "averageReceiptMetrics": _average_metrics(items, "receiptMetrics"),
        "answerStatusCounts": _answer_status_counts(items),
        "averageAnswerQuality": _average_answer_quality(items),
        "evidence": _aggregation_evidence(items),
    }


def _aggregation_evidence(items: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "scenarioIds": _sorted_unique_strings(item.get("scenarioId") for item in items),
        "scenarioPaths": _sorted_unique_strings(item.get("scenarioPath") for item in items),
        "questionIds": _sorted_unique_strings(item.get("questionId") for item in items),
        "sourceReceiptPaths": _sorted_unique_strings(
            item.get("sourceReceiptPath") for item in items
        ),
        "granularity": "scenario-receipt",
    }


def _sorted_unique_strings(values: object) -> list[str]:
    return sorted({value for value in values if isinstance(value, str) and value})


def _average_metrics(items: list[dict[str, Any]], section: str) -> dict[str, float]:
    totals: dict[str, float] = defaultdict(float)
    counts: dict[str, int] = defaultdict(int)
    for item in items:
        metrics = dict_value(item.get(section))
        for key, value in metrics.items():
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                continue
            totals[key] += float(value)
            counts[key] += 1
    return {
        key: round(totals[key] / counts[key], 6)
        for key in sorted(totals)
        if counts[key] > 0
    }


def _answer_status_counts(items: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in items:
        status = require_str(dict_value(item.get("answerMetrics")), "finalAnswerStatus", "unknown")
        counts[status] = counts.get(status, 0) + 1
    return dict(sorted(counts.items()))


def _average_answer_quality(items: list[dict[str, Any]]) -> float | None:
    values = []
    for item in items:
        value = dict_value(item.get("answerMetrics")).get("answerQualityJudgment")
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            continue
        values.append(float(value))
    if not values:
        return None
    return round(sum(values) / len(values), 6)


def _variant_recommendations(
    observed: list[dict[str, Any]],
    aggregations: list[dict[str, Any]],
) -> dict[str, Any]:
    overall_rank = _with_comparison_deltas(_overall_variant_rank(observed))
    bucket_winners = _bucket_winners(aggregations)
    return {
        "status": "ready" if overall_rank else "collecting",
        "rankingMetric": "answerQualityThenElapsedMs",
        "overallWinner": overall_rank[0] if overall_rank else None,
        "overallRank": overall_rank,
        "localEvidenceAblation": _variant_focus(overall_rank, "no-local-evidence"),
        "bucketWinners": bucket_winners,
        "adaptivePolicy": _adaptive_policy(overall_rank, bucket_winners),
    }


def _overall_variant_rank(observed: list[dict[str, Any]]) -> list[dict[str, Any]]:
    buckets: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in observed:
        buckets[str(item["ablationVariant"])].append(item)
    rank = [
        _variant_recommendation_entry(variant, items)
        for variant, items in sorted(buckets.items())
    ]
    return sorted(rank, key=_recommendation_sort_key)


def _variant_recommendation_entry(
    variant: str,
    items: list[dict[str, Any]],
) -> dict[str, Any]:
    receipt_metrics = _average_metrics(items, "receiptMetrics")
    answer_quality = _average_answer_quality(items)
    return {
        "ablationVariant": variant,
        "resultCount": len(items),
        "averageAnswerQuality": answer_quality,
        "averageReceiptMetrics": receipt_metrics,
        "averageElapsedMs": receipt_metrics.get("elapsedMs"),
        "averageAspCommandCount": receipt_metrics.get("aspCommandCount"),
        "averageStdoutBytes": receipt_metrics.get("stdoutBytes"),
    }


def _with_comparison_deltas(rank: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not rank:
        return []
    winner = rank[0]
    return [
        {
            **entry,
            "rank": index,
            "comparisonDeltas": {
                "answerQualityDeltaFromWinner": _delta(
                    entry.get("averageAnswerQuality"),
                    winner.get("averageAnswerQuality"),
                ),
                "elapsedMsDeltaFromWinner": _delta(
                    entry.get("averageElapsedMs"),
                    winner.get("averageElapsedMs"),
                ),
                "aspCommandCountDeltaFromWinner": _delta(
                    entry.get("averageAspCommandCount"),
                    winner.get("averageAspCommandCount"),
                ),
                "stdoutBytesDeltaFromWinner": _delta(
                    entry.get("averageStdoutBytes"),
                    winner.get("averageStdoutBytes"),
                ),
            },
        }
        for index, entry in enumerate(rank, start=1)
    ]


def _variant_focus(
    rank: list[dict[str, Any]],
    variant: str,
) -> dict[str, Any] | None:
    for entry in rank:
        if entry.get("ablationVariant") == variant:
            return entry
    return None


def _bucket_winners(aggregations: list[dict[str, Any]]) -> list[dict[str, Any]]:
    buckets: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for item in aggregations:
        buckets[
            (
                str(item["language"]),
                str(item["package"]),
                str(item["depthBucket"]),
            )
        ].append(item)
    winners = []
    for key, items in sorted(buckets.items()):
        winner = sorted(items, key=_recommendation_sort_key)[0]
        language, package, depth_bucket = key
        winners.append(
            {
                "language": language,
                "package": package,
                "depthBucket": depth_bucket,
                "ablationVariant": winner["ablationVariant"],
                "candidateCount": len(items),
                "resultCount": winner["resultCount"],
                "averageAnswerQuality": winner.get("averageAnswerQuality"),
                "averageElapsedMs": _metric_number(winner, "elapsedMs"),
                "averageAspCommandCount": _metric_number(winner, "aspCommandCount"),
                "averageStdoutBytes": _metric_number(winner, "stdoutBytes"),
                "evidence": dict_value(winner.get("evidence")),
            }
        )
    return winners


def _adaptive_policy(
    overall_rank: list[dict[str, Any]],
    bucket_winners: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    if not overall_rank:
        return []
    rules = [
        {
            "id": "evaluate-default-query-first-stage-policy",
            "priority": "p1",
            "ablationVariant": overall_rank[0]["ablationVariant"],
            "action": (
                "evaluate the overall fastest safe variant as the default "
                "query-first-stage policy candidate"
            ),
            "bucketCount": len(bucket_winners),
        }
    ]
    by_variant: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for winner in bucket_winners:
        by_variant[str(winner["ablationVariant"])].append(winner)
    for variant, winners in sorted(by_variant.items()):
        rules.append(
            {
                "id": f"prefer-{variant}-for-winning-buckets",
                "priority": "p1",
                "ablationVariant": variant,
                "action": "prefer this variant for matching language/package/depth buckets",
                "bucketCount": len(winners),
                "buckets": [
                    {
                        "language": item["language"],
                        "package": item["package"],
                        "depthBucket": item["depthBucket"],
                    }
                    for item in winners
                ],
            }
        )
    return rules


def _recommendation_sort_key(item: dict[str, Any]) -> tuple[float, float, float, float, str]:
    quality = item.get("averageAnswerQuality")
    quality_value = _number(quality, -1.0)
    return (
        -quality_value,
        _metric_number(item, "elapsedMs"),
        _metric_number(item, "aspCommandCount"),
        _metric_number(item, "stdoutBytes"),
        str(item.get("ablationVariant", "")),
    )


def _metric_number(item: dict[str, Any], key: str) -> float:
    metrics = dict_value(item.get("averageReceiptMetrics"))
    return _number(metrics.get(key), float("inf"))


def _number(value: object, default: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return default
    return float(value)


def _delta(value: object, baseline: object) -> float | None:
    if (
        isinstance(value, bool)
        or isinstance(baseline, bool)
        or not isinstance(value, (int, float))
        or not isinstance(baseline, (int, float))
    ):
        return None
    return round(float(value) - float(baseline), 6)


def _findings(
    expected_variant_runs: int,
    observed: list[dict[str, Any]],
    aggregations: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    findings = []
    if len(observed) < expected_variant_runs:
        findings.append(
            _finding(
                "missing-variant-results",
                "warning",
                "collect remaining optimization variant receipts before calibration",
                {
                    "expectedVariantRunCount": expected_variant_runs,
                    "observedVariantRunCount": len(observed),
                    "missingVariantRunCount": expected_variant_runs - len(observed),
                },
            )
        )
    baseline_derived = _metric_source_count(observed, "source-sandtable-receipt")
    if baseline_derived:
        findings.append(
            _finding(
                "baseline-derived-variant-results",
                "warning",
                "collect ablation-specific variant receipts before calibration",
                {"variantResultCount": baseline_derived},
            )
        )
    source_equivalent = _metric_source_count(
        observed, "source-equivalent-variant-receipt"
    )
    if source_equivalent:
        findings.append(
            _finding(
                "source-equivalent-variant-results",
                "warning",
                "rerun variant receipts because they are identical to baseline receipts",
                {"variantResultCount": source_equivalent},
            )
        )
    fallback_derived = _metric_source_count(observed, "fallback")
    if fallback_derived:
        findings.append(
            _finding(
                "fallback-derived-variant-results",
                "warning",
                "replace fallback variant metrics with real sandtable receipts",
                {"variantResultCount": fallback_derived},
            )
        )
    weak = [
        item
        for item in aggregations
        if (item.get("averageAnswerQuality") or 0) < 0.75
    ]
    if weak:
        findings.append(
            _finding(
                "weak-answer-quality",
                "warning",
                "review graph-turbo variants with weak average answer quality",
                {"aggregationCount": len(weak)},
            )
        )
    return findings


def _metric_source_count(observed: list[dict[str, Any]], source: str) -> int:
    return sum(
        1
        for item in observed
        if dict_value(item.get("receiptMetrics")).get("metricSource") == source
    )


def _finding(
    kind: str,
    severity: str,
    message: str,
    metrics: dict[str, Any],
) -> dict[str, Any]:
    return {
        "kind": kind,
        "severity": severity,
        "message": message,
        "metrics": metrics,
    }


def _summary(
    expected_variant_runs: int,
    observed: list[dict[str, Any]],
    findings: list[dict[str, Any]],
) -> dict[str, Any]:
    missing = max(expected_variant_runs - len(observed), 0)
    blocked = any(
        finding.get("kind")
        in {
            "baseline-derived-variant-results",
            "source-equivalent-variant-results",
            "fallback-derived-variant-results",
        }
        for finding in findings
    )
    return {
        "status": (
            "analyzed"
            if expected_variant_runs and missing == 0 and not blocked
            else "collecting"
        ),
        "expectedVariantRunCount": expected_variant_runs,
        "observedVariantRunCount": len(observed),
        "missingVariantRunCount": missing,
        "findingCount": len(findings),
    }


def _improvement_plan(
    findings: list[dict[str, Any]],
    aggregations: list[dict[str, Any]],
    recommendations: dict[str, Any],
) -> list[dict[str, Any]]:
    if any(
        item["kind"]
        in {
            "missing-variant-results",
            "baseline-derived-variant-results",
            "source-equivalent-variant-results",
            "fallback-derived-variant-results",
        }
        for item in findings
    ):
        return [
            {
                "id": "collect-variant-receipts",
                "priority": "p0",
                "action": "run ablation-specific sandtable receipts before graph calibration",
                "targetMetric": "runsNeedingVariantReceipt",
            }
        ]
    return [
        {
            "id": "calibrate-query-first-stage",
            "priority": "p1",
            "action": "calibrate query-first-stage policy from variant recommendations",
            "targetMetric": "variantRecommendations.overallWinner.averageElapsedMs",
            "aggregationCount": len(aggregations),
            "overallWinner": dict_value(recommendations.get("overallWinner")).get(
                "ablationVariant"
            ),
            "rankedVariantCount": len(
                list_value(recommendations.get("overallRank"))
            ),
            "localEvidenceAblationRank": dict_value(
                recommendations.get("localEvidenceAblation")
            ).get("rank"),
            "bucketWinnerCount": len(list_value(recommendations.get("bucketWinners"))),
        }
    ]


def _source_report_chain(report_chain: dict[str, Any]) -> dict[str, Any]:
    rollup = dict_value(report_chain.get("rollup"))
    batch = dict_value(report_chain.get("optimizationBatch"))
    return {
        "schemaId": report_chain.get("schemaId"),
        "packetKind": report_chain.get("packetKind"),
        "optimizationRunCount": rollup.get("optimizationRunCount"),
        "optimizationVariantRunCount": rollup.get("optimizationVariantRunCount"),
        "targetGraphPhase": batch.get("targetGraphPhase"),
        "nextStage": batch.get("nextStage"),
    }


def _variant_run_id(item: dict[str, Any], variant: str) -> str:
    return f"{require_str(item, 'runId', 'unknown')}:{variant}"
