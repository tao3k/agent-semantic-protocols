"""Report aggregation for adaptive large-library simulation runs."""

from __future__ import annotations

from typing import Any

from .large_library_adaptive_signals import (
    _line_payload,
    _line_value,
    _matched_query_terms,
    _owner_arg,
    _query_arg,
    _query_terms,
    _selector_arg,
)
from .utils import dict_value, list_value, require_str


def _summary(run_reports: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "runCount": len(run_reports),
        "statusCounts": _counts(run_reports, "status"),
        "languageCounts": _counts(run_reports, "language"),
        "variantCounts": _counts(run_reports, "ablationVariant"),
        "queryQualityCounts": _signal_counts(run_reports, "queryQuality"),
        "packageCohesionCounts": _signal_counts(run_reports, "packageCohesion"),
        "recommendedNextCounts": _signal_counts(run_reports, "recommendedNext"),
        "thirdStepActionCounts": _third_step_counts(run_reports, "action"),
        "ownerItemsRecoveryCounts": _third_step_counts(run_reports, "ownerItemsRecovery"),
        "ownerItemsTransitionCounts": _third_step_counts(
            run_reports, "ownerItemsTransition"
        ),
        "selectorQualityCounts": _third_step_counts(run_reports, "selectorQuality"),
        "finalStepActionCounts": _final_step_counts(run_reports, "action"),
        "finalRecommendedNextCounts": _final_step_counts(run_reports, "recommendedNext"),
        "finalOwnerItemsRecoveryCounts": _final_step_counts(
            run_reports, "ownerItemsRecovery"
        ),
        "finalOwnerItemsTransitionCounts": _final_step_counts(
            run_reports, "ownerItemsTransition"
        ),
        "finalSelectorQualityCounts": _final_step_counts(
            run_reports, "selectorQuality"
        ),
        "recoveryProbeActionCounts": _recovery_probe_counts(run_reports, "action"),
        "recoveryProbeOwnerItemsRecoveryCounts": _recovery_probe_counts(
            run_reports, "ownerItemsRecovery"
        ),
        "recoveryProbeOwnerItemsTransitionCounts": _recovery_probe_counts(
            run_reports, "ownerItemsTransition"
        ),
        "recoveryProbeSelectorQualityCounts": _recovery_probe_counts(
            run_reports, "selectorQuality"
        ),
        "totalCommandCount": sum(int(item.get("commandCount", 0)) for item in run_reports),
        "totalElapsedMs": sum(int(item.get("elapsedMs", 0)) for item in run_reports),
        "totalStdoutBytes": sum(int(item.get("stdoutBytes", 0)) for item in run_reports),
    }


def _variant_summaries(run_reports: list[dict[str, Any]]) -> list[dict[str, Any]]:
    variants = sorted({require_str(item, "ablationVariant", "unknown") for item in run_reports})
    return [_variant_summary(variant, run_reports) for variant in variants]


def _variant_summary(variant: str, run_reports: list[dict[str, Any]]) -> dict[str, Any]:
    items = [item for item in run_reports if item.get("ablationVariant") == variant]
    return {
        "ablationVariant": variant,
        "runCount": len(items),
        "statusCounts": _counts(items, "status"),
        "queryQualityCounts": _signal_counts(items, "queryQuality"),
        "packageCohesionCounts": _signal_counts(items, "packageCohesion"),
        "recommendedNextCounts": _signal_counts(items, "recommendedNext"),
        "thirdStepActionCounts": _third_step_counts(items, "action"),
        "ownerItemsRecoveryCounts": _third_step_counts(items, "ownerItemsRecovery"),
        "ownerItemsTransitionCounts": _third_step_counts(items, "ownerItemsTransition"),
        "selectorQualityCounts": _third_step_counts(items, "selectorQuality"),
        "finalStepActionCounts": _final_step_counts(items, "action"),
        "finalRecommendedNextCounts": _final_step_counts(items, "recommendedNext"),
        "finalOwnerItemsRecoveryCounts": _final_step_counts(items, "ownerItemsRecovery"),
        "finalOwnerItemsTransitionCounts": _final_step_counts(
            items, "ownerItemsTransition"
        ),
        "finalSelectorQualityCounts": _final_step_counts(items, "selectorQuality"),
        "recoveryProbeActionCounts": _recovery_probe_counts(items, "action"),
        "recoveryProbeOwnerItemsRecoveryCounts": _recovery_probe_counts(
            items, "ownerItemsRecovery"
        ),
        "recoveryProbeOwnerItemsTransitionCounts": _recovery_probe_counts(
            items, "ownerItemsTransition"
        ),
        "recoveryProbeSelectorQualityCounts": _recovery_probe_counts(
            items, "selectorQuality"
        ),
        "averageElapsedMs": _average([int(item.get("elapsedMs", 0)) for item in items]),
        "averageStdoutBytes": _average([int(item.get("stdoutBytes", 0)) for item in items]),
    }


def _algorithm_improvement_plan(run_reports: list[dict[str, Any]]) -> list[dict[str, Any]]:
    items = []
    query_quality = _signal_counts(run_reports, "queryQuality")
    cohesion = _signal_counts(run_reports, "packageCohesion")
    next_counts = _signal_counts(run_reports, "recommendedNext")
    if query_quality.get("low", 0):
        items.append(
            _plan_item(
                "seed-query-quality",
                query_quality["low"],
                "Split prompt terms into owner, symbol/dependency, and behavior clauses before graph seeding.",
            )
        )
    if cohesion.get("low", 0) or cohesion.get("medium", 0):
        items.append(
            _plan_item(
                "package-cohesion",
                cohesion.get("low", 0) + cohesion.get("medium", 0),
                "Raise package-local owners before global recall nodes in the first-stage frontier.",
            )
        )
    non_selector = sum(
        count
        for key, count in next_counts.items()
        if "query-selector" not in key and key != "unknown"
    )
    if non_selector:
        items.append(
            _plan_item(
                "selector-precision",
                non_selector,
                "Boost parser-owned owner-items/query-selector transitions when pipe has enough clause coverage.",
            )
        )
    recovery_counts = _third_step_counts(run_reports, "ownerItemsRecovery")
    scoped_recovery = recovery_counts.get("scoped-rg-query", 0) + recovery_counts.get(
        "no-hit-unscoped", 0
    )
    if scoped_recovery:
        items.append(
            _plan_item(
                "owner-items-recovery",
                scoped_recovery,
                "Use owner-items recovery cases to penalize owner candidates with weak local item evidence and prefer owners with denser parser/finder-local hits.",
            )
        )
    selector_quality = _third_step_counts(run_reports, "selectorQuality")
    weak_selectors = sum(
        count
        for key, count in selector_quality.items()
        if key not in {"source-selector", "not-selector-ready", "unknown"}
    )
    if weak_selectors:
        items.append(
            _plan_item(
                "selector-quality",
                weak_selectors,
                "Separate executable selectors from semantically strong implementation selectors, then penalize secondary artifacts and weak query-axis coverage in owner-items ranking.",
            )
        )
    final_next = _final_step_counts(run_reports, "recommendedNext")
    fd_scoped_rg = final_next.get("A1.scoped-rg-query", 0)
    if fd_scoped_rg:
        items.append(
            _plan_item(
                "final-frontier-convergence",
                fd_scoped_rg,
                "Use final-step fd/scoped-rg loops to promote stronger owner-items convergence from path candidates, preserving raw query axes across rg-to-fd handoff instead of repeating lexical narrowing.",
            )
        )
    return items


def _plan_item(item_id: str, evidence_count: int, action: str) -> dict[str, Any]:
    return {
        "id": f"graph-turbo.{item_id}",
        "targetGraphPhase": "query-first-stage",
        "evidenceRunCount": evidence_count,
        "recommendedAction": action,
    }


def _owner_items_recovery_cases(run_reports: list[dict[str, Any]]) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for report in run_reports:
        signals = dict_value(report.get("thirdStepSignals"))
        recovery = require_str(signals, "ownerItemsRecovery", "unknown")
        if recovery in {"not-owner-items", "not-run", "unknown"}:
            continue
        commands = list_value(report.get("commands"))
        third = commands[2] if len(commands) >= 3 and isinstance(commands[2], dict) else {}
        argv = [str(value) for value in list_value(third.get("argv"))]
        stdout = str(third.get("stdout", ""))
        project = dict_value(report.get("project"))
        cases.append(
            {
                "runId": require_str(report, "runId", "unknown"),
                "scenarioId": require_str(report, "scenarioId", "unknown"),
                "questionId": require_str(report, "questionId", "unknown"),
                "language": require_str(report, "language", "unknown"),
                "ablationVariant": require_str(report, "ablationVariant", "unknown"),
                "projectName": require_str(project, "name", "unknown"),
                "recovery": recovery,
                "owner": _owner_arg(argv),
                "query": _query_arg(argv),
                "reason": _line_value(stdout, "reason"),
                "nextCommand": _line_payload(stdout, "nextCommand"),
            }
        )
    return cases


def _selector_quality_cases(run_reports: list[dict[str, Any]]) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for report in run_reports:
        cases.extend(_selector_quality_cases_for_step(report, "thirdStepSignals", 2))
        cases.extend(_selector_quality_cases_for_step(report, "recoveryProbeSignals", 4))
    return cases


def _selector_quality_cases_for_step(
    report: dict[str, Any],
    signal_key: str,
    command_index: int,
) -> list[dict[str, Any]]:
    signals = dict_value(report.get(signal_key))
    selector_quality = require_str(signals, "selectorQuality", "not-selector-ready")
    if selector_quality == "not-selector-ready":
        return []
    commands = list_value(report.get("commands"))
    command = (
        commands[command_index]
        if len(commands) > command_index and isinstance(commands[command_index], dict)
        else {}
    )
    argv = [str(value) for value in list_value(command.get("argv"))]
    stdout = str(command.get("stdout", ""))
    owner = _owner_arg(argv)
    query = _query_arg(argv)
    next_command = _line_payload(stdout, "nextCommand")
    query_terms = _query_terms(query)
    matched = _matched_query_terms(query_terms, owner, next_command, stdout)
    project = dict_value(report.get("project"))
    return [
        {
            "runId": require_str(report, "runId", "unknown"),
            "scenarioId": require_str(report, "scenarioId", "unknown"),
            "questionId": require_str(report, "questionId", "unknown"),
            "language": require_str(report, "language", "unknown"),
            "ablationVariant": require_str(report, "ablationVariant", "unknown"),
            "projectName": require_str(project, "name", "unknown"),
            "selectorQuality": selector_quality,
            "owner": owner,
            "query": query,
            "selector": _selector_arg(next_command),
            "matchedQueryTerms": matched,
            "missingQueryTerms": [term for term in query_terms if term not in set(matched)],
            "reason": _line_value(stdout, "reason"),
            "nextCommand": next_command,
        }
    ]


def _counts(items: list[dict[str, Any]], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in items:
        value = require_str(item, key, "unknown")
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def _signal_counts(items: list[dict[str, Any]], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in items:
        value = require_str(dict_value(item.get("pipeSignals")), key, "unknown")
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def _third_step_counts(items: list[dict[str, Any]], key: str) -> dict[str, int]:
    return _step_signal_counts(items, "thirdStepSignals", key)


def _final_step_counts(items: list[dict[str, Any]], key: str) -> dict[str, int]:
    return _step_signal_counts(items, "finalStepSignals", key)


def _recovery_probe_counts(items: list[dict[str, Any]], key: str) -> dict[str, int]:
    return _step_signal_counts(items, "recoveryProbeSignals", key)


def _step_signal_counts(
    items: list[dict[str, Any]], signal_key: str, key: str
) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in items:
        value = require_str(dict_value(item.get(signal_key)), key, "unknown")
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def _average(values: list[int]) -> float:
    return round(sum(values) / len(values), 4) if values else 0.0
