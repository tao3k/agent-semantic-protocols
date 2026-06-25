"""Batch builder for large-library optimization variant result packets."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from collections import Counter
from typing import Any

from .large_library_variant_result import build_large_library_variant_result
from .utils import dict_value, list_value, optional_int, require_str


def build_large_library_variant_batch(
    report_chain: dict[str, Any],
    *,
    receipt_metrics: dict[str, Any] | None = None,
    answer_metrics: dict[str, Any] | None = None,
    source_receipt_path: str | None = None,
    sandtable_receipt: dict[str, Any] | None = None,
    variant_sandtable_receipts: Mapping[str, dict[str, Any]] | None = None,
    variant_source_receipt_paths: Mapping[str, str] | None = None,
) -> list[dict[str, Any]]:
    """Build one variant-result packet for every report-chain variant run."""

    baseline = _scenario_receipts(sandtable_receipt)
    variants = _variant_scenario_receipts(variant_sandtable_receipts or {})
    return [
        _build_packet(
            report_chain,
            run=run,
            variant=variant,
            receipt_metrics=dict(receipt_metrics or {}),
            answer_metrics=dict(answer_metrics or {}),
            baseline_receipts=baseline,
            variant_receipts=variants,
            source_receipt_path=source_receipt_path,
            variant_source_receipt_paths=dict(variant_source_receipt_paths or {}),
        )
        for run, variant in _variant_run_specs(report_chain)
    ]


def _variant_run_specs(report_chain: dict[str, Any]) -> Iterable[tuple[dict[str, Any], str]]:
    for run in list_value(report_chain.get("optimizationMatrix")):
        yield from _run_variant_specs(run)


def _run_variant_specs(run: object) -> tuple[tuple[dict[str, Any], str], ...]:
    if not isinstance(run, dict):
        return ()
    run_id = _run_id(run)
    if run_id is None:
        return ()
    return tuple((run, variant) for variant in _variants(run))


def _run_id(run: dict[str, Any]) -> str | None:
    run_id = run.get("runId")
    return run_id if isinstance(run_id, str) and run_id else None


def _variants(run: dict[str, Any]) -> tuple[str, ...]:
    return tuple(
        variant
        for variant in list_value(run.get("ablationVariants"))
        if isinstance(variant, str) and variant
    )


def _scenario_receipts(
    sandtable_receipt: dict[str, Any] | None,
) -> dict[str, dict[str, Any]]:
    if sandtable_receipt is None:
        return {}
    return {
        require_str(scenario, "id", "unknown"): scenario
        for scenario in _scenario_receipt_items(sandtable_receipt)
    }


def _scenario_receipt_items(
    sandtable_receipt: dict[str, Any],
) -> tuple[dict[str, Any], ...]:
    return tuple(
        scenario
        for scenario in list_value(sandtable_receipt.get("scenarios"))
        if isinstance(scenario, dict) and isinstance(scenario.get("id"), str)
    )


def _variant_scenario_receipts(
    receipts: Mapping[str, dict[str, Any]],
) -> dict[str, dict[str, dict[str, Any]]]:
    return {key: _scenario_receipts(receipt) for key, receipt in receipts.items()}


def _build_packet(
    report_chain: dict[str, Any],
    *,
    run: dict[str, Any],
    variant: str,
    receipt_metrics: dict[str, Any],
    answer_metrics: dict[str, Any],
    baseline_receipts: dict[str, dict[str, Any]],
    variant_receipts: dict[str, dict[str, dict[str, Any]]],
    source_receipt_path: str | None,
    variant_source_receipt_paths: dict[str, str],
) -> dict[str, Any]:
    variant_run_id = f"{require_str(run, 'runId', 'unknown')}:{variant}"
    scenario_id = require_str(run, "scenarioId", "unknown")
    scenario = _scenario_for_variant(
        variant_receipts,
        variant_run_id=variant_run_id,
        variant=variant,
        scenario_id=scenario_id,
    )
    baseline_scenario = baseline_receipts.get(scenario_id)
    metric_source = _metric_source_for_variant_scenario(scenario, baseline_scenario)
    scenario = scenario or baseline_scenario
    packet_source_receipt_path = (
        variant_source_receipt_paths.get(variant_run_id)
        or variant_source_receipt_paths.get(variant)
        or source_receipt_path
    )
    return build_large_library_variant_result(
        report_chain,
        variant_run_id=variant_run_id,
        receipt_metrics=_receipt_metrics_for_scenario(
            receipt_metrics,
            scenario,
            metric_source=metric_source if scenario is not None else "fallback",
        ),
        answer_metrics=_answer_metrics_for_scenario(answer_metrics, scenario),
        source_receipt_path=packet_source_receipt_path,
    )


def _scenario_for_variant(
    variant_receipts: dict[str, dict[str, dict[str, Any]]],
    *,
    variant_run_id: str,
    variant: str,
    scenario_id: str,
) -> dict[str, Any] | None:
    exact = variant_receipts.get(variant_run_id, {}).get(scenario_id)
    if exact is not None:
        return exact
    return variant_receipts.get(variant, {}).get(scenario_id)


def _metric_source_for_variant_scenario(
    variant_scenario: dict[str, Any] | None,
    baseline_scenario: dict[str, Any] | None,
) -> str:
    if variant_scenario is None:
        return "source-sandtable-receipt"
    if baseline_scenario is not None and _scenario_equivalent_for_variant(
        variant_scenario, baseline_scenario
    ):
        return "source-equivalent-variant-receipt"
    return "variant-sandtable-receipt"


def _scenario_equivalent_for_variant(
    candidate: dict[str, Any],
    baseline: dict[str, Any],
) -> bool:
    return _without_volatile_timing(candidate) == _without_volatile_timing(baseline)


def _without_volatile_timing(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _without_volatile_timing(item)
            for key, item in value.items()
            if key != "elapsedMs"
        }
    if isinstance(value, list):
        return [_without_volatile_timing(item) for item in value]
    return value


def _receipt_metrics_for_scenario(
    fallback: dict[str, Any],
    scenario: dict[str, Any] | None,
    *,
    metric_source: str,
) -> dict[str, Any]:
    if scenario is None:
        return {**fallback, "metricSource": "fallback"}
    derived = _derive_receipt_metrics(scenario)
    derived["metricSource"] = metric_source
    return {**fallback, **derived}


def _answer_metrics_for_scenario(
    fallback: dict[str, Any],
    scenario: dict[str, Any] | None,
) -> dict[str, Any]:
    if scenario is None:
        return fallback
    derived = _derive_answer_metrics(scenario)
    return {**fallback, **derived}


def _derive_receipt_metrics(scenario: dict[str, Any]) -> dict[str, Any]:
    steps = _steps(scenario)
    commands = [_command(step) for step in steps]
    flow_metrics = dict_value(scenario.get("flowMetrics"))
    return {
        "aspCommandCount": _command_count(scenario, steps),
        "searchCommandCount": sum(1 for command in commands if _is_search(command)),
        "queryCommandCount": sum(1 for command in commands if _is_query(command)),
        "repeatedCommandCount": _repeated_command_count(commands),
        "commandsToFirstUsefulLocator": _commands_to_first_useful_locator(steps),
        "frontierFollowRate": _frontier_follow_rate(scenario, steps),
        "rawReadFallbackCount": sum(1 for command in commands if _is_raw_read(command)),
        "duplicateSelectorCount": _duplicate_selector_count(commands),
        "sameOwnerScanCount": _same_owner_scan_count(commands),
        "elapsedMs": optional_int(flow_metrics.get("elapsedMs")) or 0,
        "stdoutBytes": optional_int(flow_metrics.get("stdoutBytes")) or 0,
        "stderrBytes": optional_int(flow_metrics.get("stderrBytes")) or 0,
        "queryTopologyMembershipCandidateCount": optional_int(
            flow_metrics.get("queryTopologyMembershipCandidateCount")
        )
        or 0,
        "queryTopologyMembershipCoverageRate": _optional_float(
            flow_metrics.get("queryTopologyMembershipCoverageRate")
        ),
        "queryTopologyMembershipDriftRate": _optional_float(
            flow_metrics.get("queryTopologyMembershipDriftRate")
        ),
        "queryTopologyMembershipDelta": optional_int(
            flow_metrics.get("queryTopologyMembershipDelta")
        )
        or 0,
    }


def _derive_answer_metrics(scenario: dict[str, Any]) -> dict[str, Any]:
    errors = _scenario_errors(scenario)
    status = require_str(scenario, "status", "unknown")
    answered = status == "pass" and not errors
    return {
        "finalAnswerStatus": "answered" if answered else "failed",
        "answerQualityJudgment": 1.0 if answered else 0.0,
        "missingEvidenceCount": len(errors),
        "wrongOwnerCount": 0 if answered else 1,
    }


def _steps(scenario: dict[str, Any]) -> tuple[dict[str, Any], ...]:
    return tuple(
        step for step in list_value(scenario.get("steps")) if isinstance(step, dict)
    )


def _command(step: dict[str, Any]) -> tuple[str, ...]:
    return tuple(str(part) for part in list_value(step.get("command")))


def _command_count(scenario: dict[str, Any], steps: tuple[dict[str, Any], ...]) -> int:
    flow_metrics = dict_value(scenario.get("flowMetrics"))
    return optional_int(flow_metrics.get("commands")) or len(steps)


def _is_search(command: tuple[str, ...]) -> bool:
    return "search" in command


def _is_query(command: tuple[str, ...]) -> bool:
    return "query" in command


def _repeated_command_count(commands: list[tuple[str, ...]]) -> int:
    counts = Counter(commands)
    return sum(count - 1 for count in counts.values() if count > 1)


def _commands_to_first_useful_locator(steps: tuple[dict[str, Any], ...]) -> int:
    for index, step in enumerate(steps, start=1):
        if _is_useful_locator_step(step):
            return index
    return len(steps) if steps else 0


def _is_useful_locator_step(step: dict[str, Any]) -> bool:
    step_id = require_str(step, "id", "")
    return step_id != "prime" and _is_search(_command(step))


def _frontier_follow_rate(
    scenario: dict[str, Any],
    steps: tuple[dict[str, Any], ...],
) -> float:
    if _scenario_errors(scenario):
        return 0.0
    if not steps:
        return 0.0
    followed_steps = sum(1 for step in steps if _follows_frontier(_command(step)))
    return followed_steps / len(steps)


def _follows_frontier(command: tuple[str, ...]) -> bool:
    return _is_search(command) or _is_query(command)


def _scenario_errors(scenario: dict[str, Any]) -> tuple[Any, ...]:
    step_errors = tuple(error for step in _steps(scenario) for error in _step_errors(step))
    return (*list_value(scenario.get("errors")), *step_errors)


def _step_errors(step: dict[str, Any]) -> tuple[Any, ...]:
    return tuple(list_value(step.get("errors")))


def _is_raw_read(command: tuple[str, ...]) -> bool:
    if not command or command[0] == "asp":
        return False
    return command[0] in {"cat", "sed", "grep", "awk", "head", "tail", "less"}


def _duplicate_selector_count(commands: list[tuple[str, ...]]) -> int:
    selectors = [
        command[index + 1]
        for command in commands
        for index, part in enumerate(command[:-1])
        if part == "--selector"
    ]
    counts = Counter(selectors)
    return sum(count - 1 for count in counts.values() if count > 1)


def _same_owner_scan_count(commands: list[tuple[str, ...]]) -> int:
    owners = [
        command[index + 1]
        for command in commands
        for index, part in enumerate(command[:-1])
        if part == "owner"
    ]
    counts = Counter(owners)
    return sum(count - 1 for count in counts.values() if count > 1)


def _optional_float(value: Any) -> float:
    if isinstance(value, bool) or value is None:
        return 0.0
    if isinstance(value, int | float):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return 0.0
    return 0.0
