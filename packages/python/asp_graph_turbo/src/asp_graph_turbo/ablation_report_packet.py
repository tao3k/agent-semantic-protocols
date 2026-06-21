"""Ablation comparison report builder for graph turbo calibration."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from .ablation_cli import ABLATION_VARIANTS, build_ablation_set
from .benchmark_cli import benchmark_packet_with_result


def build_ablation_report(
    packet: Mapping[str, object],
    *,
    variants: Sequence[str] = ABLATION_VARIANTS,
    runs: int,
    warmup_runs: int,
    cache_mode: str,
    profile: str | None = None,
    seed: Sequence[str] = (),
    limit: int | None = None,
    quality_config: Mapping[str, object] | None = None,
) -> dict[str, object]:
    baseline = _variant_measurement(
        packet,
        variant="full",
        changes={"description": "unchanged graph-turbo request packet"},
        runs=runs,
        warmup_runs=warmup_runs,
        cache_mode=cache_mode,
        profile=profile,
        seed=seed,
        limit=limit,
    )
    ablation_set = build_ablation_set(packet, variants)
    entries = [
        _variant_report(
            baseline,
            variant_packet,
            runs=runs,
            warmup_runs=warmup_runs,
            cache_mode=cache_mode,
            profile=profile,
            seed=seed,
            limit=limit,
        )
        for variant_packet in _variant_entries(ablation_set)
    ]
    summary = _summary(entries)
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-ablation-report",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-ablation-report",
        "sourceProfile": packet.get("profile"),
        "baselineVariant": "full",
        "runs": runs,
        "warmupRuns": warmup_runs,
        "cacheMode": cache_mode,
        "summary": summary,
        "qualityGate": _quality_gate(entries, summary, quality_config or {}),
        "variants": entries,
    }


def _variant_report(
    baseline: Mapping[str, object],
    variant_entry: Mapping[str, object],
    *,
    runs: int,
    warmup_runs: int,
    cache_mode: str,
    profile: str | None,
    seed: Sequence[str],
    limit: int | None,
) -> dict[str, object]:
    measurement = _variant_measurement(
        _mapping(variant_entry.get("packet")),
        variant=str(variant_entry.get("variant")),
        changes=_mapping(variant_entry.get("changes")),
        runs=runs,
        warmup_runs=warmup_runs,
        cache_mode=cache_mode,
        profile=profile,
        seed=seed,
        limit=limit,
    )
    measurement["comparison"] = _comparison(baseline, measurement)
    return measurement


def _variant_measurement(
    packet: Mapping[str, object],
    *,
    variant: str,
    changes: Mapping[str, object],
    runs: int,
    warmup_runs: int,
    cache_mode: str,
    profile: str | None,
    seed: Sequence[str],
    limit: int | None,
) -> dict[str, object]:
    benchmark, ranked = benchmark_packet_with_result(
        packet,
        runs=runs,
        warmup_runs=warmup_runs,
        cache_mode=cache_mode,
        profile=profile,
        seed=seed,
        limit=limit,
    )
    return {
        "variant": variant,
        "changes": dict(changes),
        "rank": list(_string_list(ranked.get("rank"))),
        "frontierNodeIds": _frontier_node_ids(ranked),
        "scores": dict(_mapping(ranked.get("scores"))),
        "benchmark": {
            "medianMs": _mapping(benchmark.get("durationMs")).get("median"),
            "p95Ms": _mapping(benchmark.get("durationMs")).get("p95"),
            "profileMatrix": _compact_profile_matrix(
                _mapping(benchmark.get("lastProfileMatrix"))
            ),
            "algorithmMetrics": dict(_mapping(benchmark.get("lastAlgorithmMetrics"))),
        },
    }


def _comparison(
    baseline: Mapping[str, object],
    variant: Mapping[str, object],
) -> dict[str, object]:
    baseline_rank = list(_string_list(baseline.get("rank")))
    variant_rank = list(_string_list(variant.get("rank")))
    overlap = len(set(baseline_rank) & set(variant_rank))
    score_delta = _score_delta_l1(
        _mapping(baseline.get("scores")),
        _mapping(variant.get("scores")),
    )
    return {
        "rankChanged": baseline_rank != variant_rank,
        "firstRankedNodeChanged": _first(baseline_rank) != _first(variant_rank),
        "rankOverlapCount": overlap,
        "rankOverlapRatio": round(overlap / len(baseline_rank), 6)
        if baseline_rank
        else 1.0,
        "missingBaselineNodeIds": [
            node_id for node_id in baseline_rank if node_id not in variant_rank
        ],
        "newRankedNodeIds": [
            node_id for node_id in variant_rank if node_id not in baseline_rank
        ],
        "scoreDeltaL1": score_delta,
        "pprIterationDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "pprIterations"
        ),
        "selectedEdgeCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "selectedEdgeCount"
        ),
        "reachableNodeCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "reachableNodeCount"
        ),
        "readMemorySuppressedDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "readMemorySuppressedCount"
        ),
        "receiptBoostDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "receiptBoostCount"
        ),
        "receiptPenaltyDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "receiptPenaltyCount"
        ),
        "transitionNonZeroDelta": _metric_delta(
            baseline, variant, "profileMatrix", "transitionNonZeroCount"
        ),
        "transitionWeightMassDelta": _metric_delta(
            baseline, variant, "profileMatrix", "transitionWeightMass"
        ),
        "querySeedPriorCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "querySeedPriorCount"
        ),
        "queryPackageCohesionCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "queryPackageCohesionCount"
        ),
        "queryPackageDriftPenaltyCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "queryPackageDriftPenaltyCount"
        ),
        "queryClauseCoverageCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "queryClauseCoverageCount"
        ),
        "queryLocalEvidenceBoostCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "queryLocalEvidenceBoostCount"
        ),
        "queryLocalEvidencePenaltyCountDelta": _metric_delta(
            baseline, variant, "algorithmMetrics", "queryLocalEvidencePenaltyCount"
        ),
    }


def _summary(entries: Sequence[Mapping[str, object]]) -> dict[str, object]:
    by_variant = {str(entry.get("variant")): entry for entry in entries}
    comparisons = [_mapping(entry.get("comparison")) for entry in entries]
    ratios = [
        value
        for value in (comparison.get("rankOverlapRatio") for comparison in comparisons)
        if isinstance(value, (int, float))
    ]
    deltas = [
        value
        for value in (comparison.get("scoreDeltaL1") for comparison in comparisons)
        if isinstance(value, (int, float))
    ]
    changed = [
        str(entry.get("variant"))
        for entry in entries
        if _mapping(entry.get("comparison")).get("rankChanged") is True
    ]
    return {
        "variantCount": len(entries),
        "rankChangedVariantCount": len(changed),
        "rankChangedVariants": changed,
        "worstRankOverlapRatio": min(ratios) if ratios else 1.0,
        "maxScoreDeltaL1": max(deltas) if deltas else 0.0,
        "topologyMembershipAblationEnabled": _topology_membership_signal(
            by_variant.get("no-topology-membership", {})
        ),
    }


def _quality_gate(
    entries: Sequence[Mapping[str, object]],
    summary: Mapping[str, object],
    config: Mapping[str, object],
) -> dict[str, object]:
    thresholds = _quality_thresholds(config)
    signals = _channel_signals(entries)
    failures: list[dict[str, object]] = []
    _check_minimum(
        failures,
        "summary.worstRankOverlapRatio",
        summary.get("worstRankOverlapRatio"),
        thresholds["minWorstRankOverlapRatio"],
    )
    max_score_delta = thresholds.get("maxScoreDeltaL1")
    if max_score_delta is not None:
        _check_maximum(
            failures,
            "summary.maxScoreDeltaL1",
            summary.get("maxScoreDeltaL1"),
            max_score_delta,
        )
    if thresholds["requireChannelSignals"] is True:
        for name, detected in signals.items():
            if detected is not True:
                _add_failure(
                    failures,
                    f"signals.{name}",
                    detected,
                    "signal detected",
                )
    return {
        "status": "pass" if not failures else "fail",
        "thresholds": thresholds,
        "signals": signals,
        "failures": failures,
    }


def _quality_thresholds(config: Mapping[str, object]) -> dict[str, object]:
    return {
        "minWorstRankOverlapRatio": _float_config(
            config, "minWorstRankOverlapRatio", 0.0
        ),
        "maxScoreDeltaL1": _optional_float_config(config, "maxScoreDeltaL1"),
        "requireChannelSignals": config.get("requireChannelSignals") is True,
    }


def _channel_signals(entries: Sequence[Mapping[str, object]]) -> dict[str, bool]:
    by_entry = {str(entry.get("variant")): entry for entry in entries}
    by_variant = {
        str(entry.get("variant")): _mapping(entry.get("comparison"))
        for entry in entries
    }
    return {
        "receipt": _receipt_signal(by_variant.get("no-receipt", {})),
        "readMemory": _delta_nonzero(
            by_variant.get("no-read-memory", {}),
            "readMemorySuppressedDelta",
        ),
        "qualityFields": _quality_fields_signal(
            by_variant.get("no-quality-fields", {})
        ),
        "providerFacts": _provider_facts_signal(
            by_variant.get("no-provider-facts", {})
        ),
        "queryFirstStage": _query_first_stage_signal(by_variant),
        "topologyMembership": _topology_membership_signal(
            by_entry.get("no-topology-membership", {})
        ),
    }


def _receipt_signal(comparison: Mapping[str, object]) -> bool:
    return (
        _delta_nonzero(comparison, "receiptBoostDelta")
        or _delta_nonzero(comparison, "receiptPenaltyDelta")
        or _score_delta_positive(comparison)
    )


def _quality_fields_signal(comparison: Mapping[str, object]) -> bool:
    return (
        _score_delta_positive(comparison)
        or _delta_nonzero(comparison, "transitionWeightMassDelta")
        or _delta_nonzero(comparison, "selectedEdgeCountDelta")
    )


def _provider_facts_signal(comparison: Mapping[str, object]) -> bool:
    return (
        comparison.get("rankChanged") is True
        or _score_delta_positive(comparison)
        or _delta_nonzero(comparison, "transitionNonZeroDelta")
    )


def _query_first_stage_signal(
    by_variant: Mapping[str, Mapping[str, object]],
) -> bool:
    return any(
        _query_variant_signal(by_variant.get(variant, {}))
        for variant in (
            "no-query-seed-prior",
            "no-package-cohesion",
            "no-query-clause-coverage",
            "no-local-evidence",
            "no-topology-membership",
        )
    )


def _topology_membership_signal(entry: Mapping[str, object]) -> bool:
    changes = _mapping(entry.get("changes"))
    policy = _mapping(changes.get("queryAdjustmentPolicy"))
    return policy.get("topologyMembership") is False


def _query_variant_signal(comparison: Mapping[str, object]) -> bool:
    return (
        comparison.get("rankChanged") is True
        or _score_delta_positive(comparison)
        or _delta_nonzero(comparison, "querySeedPriorCountDelta")
        or _delta_nonzero(comparison, "queryPackageCohesionCountDelta")
        or _delta_nonzero(comparison, "queryPackageDriftPenaltyCountDelta")
        or _delta_nonzero(comparison, "queryClauseCoverageCountDelta")
        or _delta_nonzero(comparison, "queryLocalEvidenceBoostCountDelta")
        or _delta_nonzero(comparison, "queryLocalEvidencePenaltyCountDelta")
    )


def _score_delta_positive(comparison: Mapping[str, object]) -> bool:
    value = comparison.get("scoreDeltaL1")
    return isinstance(value, (int, float)) and not isinstance(value, bool) and value > 0


def _delta_nonzero(comparison: Mapping[str, object], name: str) -> bool:
    value = comparison.get(name)
    return (
        isinstance(value, (int, float)) and not isinstance(value, bool) and value != 0
    )


def _check_minimum(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    minimum: object,
) -> None:
    value = _number(actual)
    expected = _number(minimum)
    if value is None or expected is None or value < expected:
        _add_failure(failures, field, actual, f"value >= {minimum}")


def _check_maximum(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    maximum: object,
) -> None:
    value = _number(actual)
    expected = _number(maximum)
    if value is None or expected is None or value > expected:
        _add_failure(failures, field, actual, f"value <= {maximum}")


def _add_failure(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    expected: str,
) -> None:
    failures.append({"field": field, "actual": actual, "expected": expected})


def _variant_entries(ablation_set: Mapping[str, object]) -> list[Mapping[str, object]]:
    variants = ablation_set.get("variants")
    if not isinstance(variants, list):
        return []
    return [entry for entry in variants if isinstance(entry, Mapping)]


def _frontier_node_ids(packet: Mapping[str, object]) -> list[str]:
    frontier = packet.get("frontier")
    if not isinstance(frontier, list):
        return []
    return [
        str(entry.get("nodeId"))
        for entry in frontier
        if isinstance(entry, Mapping) and entry.get("nodeId") is not None
    ]


def _compact_profile_matrix(matrix: Mapping[str, object]) -> dict[str, object]:
    return {
        key: matrix.get(key)
        for key in (
            "profile",
            "relationCount",
            "relationMatrixCount",
            "zeroEdgeRelationCount",
            "transitionNonZeroCount",
            "transitionWeightMass",
        )
    }


def _metric_delta(
    baseline: Mapping[str, object],
    variant: Mapping[str, object],
    section: str,
    name: str,
) -> float | int | None:
    baseline_value = _metric_value(baseline, section, name)
    variant_value = _metric_value(variant, section, name)
    if baseline_value is None or variant_value is None:
        return None
    delta = variant_value - baseline_value
    return int(delta) if float(delta).is_integer() else round(delta, 6)


def _metric_value(
    entry: Mapping[str, object],
    section: str,
    name: str,
) -> float | None:
    benchmark = _mapping(entry.get("benchmark"))
    value = _mapping(benchmark.get(section)).get(name)
    if isinstance(value, bool):
        return None
    return float(value) if isinstance(value, (int, float)) else None


def _score_delta_l1(
    baseline_scores: Mapping[str, object],
    variant_scores: Mapping[str, object],
) -> float:
    node_ids = set(baseline_scores) | set(variant_scores)
    total = sum(
        abs(
            _float_score(baseline_scores.get(node_id))
            - _float_score(variant_scores.get(node_id))
        )
        for node_id in node_ids
    )
    return round(total, 6)


def _float_score(value: object) -> float:
    return (
        float(value)
        if isinstance(value, (int, float)) and not isinstance(value, bool)
        else 0.0
    )


def _string_list(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(str(item) for item in value if isinstance(item, str))


def _first(values: Sequence[str]) -> str | None:
    return values[0] if values else None


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _number(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    return float(value) if isinstance(value, (int, float)) else None


def _float_config(config: Mapping[str, object], name: str, default: float) -> float:
    value = config.get(name, default)
    return float(value) if isinstance(value, (int, float)) else default


def _optional_float_config(config: Mapping[str, object], name: str) -> float | None:
    value = config.get(name)
    return float(value) if isinstance(value, (int, float)) else None
