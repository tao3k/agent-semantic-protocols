"""Failure-frontier receipt comparison for real-trigger sandtables."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .output import emit
from .receipts import load_receipt
from .report_format import quote_value
from .utils import dict_value, list_value, optional_int, require_str, string_list


@dataclass(frozen=True)
class FailureFrontierThresholds:
    min_command_reduction: float = 0.5
    max_direct_source_read_code: int = 4
    max_duplicate_selectors: int = 0
    max_same_file_window_fanout: int = 0


def compare_failure_frontier_receipt_paths(
    repo_root: Path,
    baseline_path: Path,
    candidate_path: Path,
    *,
    path_base: Path | None = None,
    expected_hot_blocks: list[str] | None = None,
    thresholds: FailureFrontierThresholds | None = None,
) -> dict[str, Any]:
    baseline = load_receipt(
        _resolve_receipt_path(repo_root, baseline_path, path_base=path_base),
        repo_root,
    )
    candidate = load_receipt(
        _resolve_receipt_path(repo_root, candidate_path, path_base=path_base),
        repo_root,
    )
    return compare_failure_frontier_receipts(
        baseline,
        candidate,
        expected_hot_blocks=expected_hot_blocks or [],
        thresholds=thresholds or FailureFrontierThresholds(),
    )


def compare_failure_frontier_receipts(
    baseline: dict[str, Any],
    candidate: dict[str, Any],
    *,
    expected_hot_blocks: list[str],
    thresholds: FailureFrontierThresholds,
) -> dict[str, Any]:
    baseline_metrics = _receipt_metrics(baseline)
    candidate_metrics = _receipt_metrics(candidate)
    frontier = _comparison_frontier(candidate, expected_hot_blocks)
    delta = _comparison_delta(baseline_metrics, candidate_metrics)
    failures = _comparison_failures(
        command_reduction_ratio=delta["commandReductionRatio"],
        candidate_metrics=candidate_metrics,
        missing_hot_blocks=frontier["missingHotBlocks"],
        thresholds=thresholds,
    )
    return _comparison_payload(
        baseline_metrics=baseline_metrics,
        candidate_metrics=candidate_metrics,
        delta=delta,
        frontier=frontier,
        thresholds=thresholds,
        failures=failures,
    )


def _comparison_frontier(
    candidate: dict[str, Any],
    expected_hot_blocks: list[str],
) -> dict[str, Any]:
    declared_failure_frontier = _declared_failure_frontier(candidate)
    declared_hot_blocks = _declared_hot_blocks(
        candidate,
        declared_failure_frontier=declared_failure_frontier,
    )
    expected_hot_blocks = expected_hot_blocks or declared_hot_blocks
    read_targets = _read_targets(candidate)
    covered = [
        target
        for target in expected_hot_blocks
        if _target_covered(target, read_targets)
    ]
    missing = [target for target in expected_hot_blocks if target not in covered]
    return {
        "expectedHotBlocks": expected_hot_blocks,
        "declaredFailureFrontier": declared_failure_frontier,
        "declaredHotBlocks": declared_hot_blocks,
        "readHotBlocks": sorted(read_targets),
        "coveredHotBlocks": covered,
        "missingHotBlocks": missing,
        "coverageRatio": _coverage_ratio(covered, expected_hot_blocks),
    }


def _comparison_delta(
    baseline_metrics: dict[str, Any],
    candidate_metrics: dict[str, Any],
) -> dict[str, Any]:
    command_reduction = (
        baseline_metrics["commandCount"] - candidate_metrics["commandCount"]
    )
    stdout_reduction = (
        baseline_metrics["stdoutBytes"] - candidate_metrics["stdoutBytes"]
    )
    return {
        "commandReduction": command_reduction,
        "commandReductionRatio": _ratio_reduction(
            baseline_metrics["commandCount"],
            candidate_metrics["commandCount"],
        ),
        "stdoutBytesReduction": stdout_reduction,
        "stdoutBytesReductionRatio": _ratio_reduction(
            baseline_metrics["stdoutBytes"],
            candidate_metrics["stdoutBytes"],
        ),
    }


def _comparison_payload(
    *,
    baseline_metrics: dict[str, Any],
    candidate_metrics: dict[str, Any],
    delta: dict[str, Any],
    frontier: dict[str, Any],
    thresholds: FailureFrontierThresholds,
    failures: list[str],
) -> dict[str, Any]:
    return {
        "schemaId": (
            "agent.semantic-protocols.semantic-sandtable-failure-frontier-comparison"
        ),
        "schemaVersion": "1",
        "status": "fail" if failures else "pass",
        "baseline": baseline_metrics,
        "candidate": candidate_metrics,
        "delta": delta,
        "frontier": frontier,
        "thresholds": _threshold_payload(thresholds),
        "failures": failures,
    }


def _threshold_payload(thresholds: FailureFrontierThresholds) -> dict[str, Any]:
    return {
        "minCommandReduction": thresholds.min_command_reduction,
        "maxDirectSourceReadCode": thresholds.max_direct_source_read_code,
        "maxDuplicateSelectors": thresholds.max_duplicate_selectors,
        "maxSameFileWindowFanout": thresholds.max_same_file_window_fanout,
    }


def print_failure_frontier_comparison(comparison: dict[str, Any]) -> None:
    baseline = dict_value(comparison.get("baseline"))
    candidate = dict_value(comparison.get("candidate"))
    delta = dict_value(comparison.get("delta"))
    frontier = dict_value(comparison.get("frontier"))
    emit(
        "[failure-frontier] "
        f"status={require_str(comparison, 'status', 'unknown')} "
        f"baselineCommands={optional_int(baseline.get('commandCount')) or 0} "
        f"candidateCommands={optional_int(candidate.get('commandCount')) or 0} "
        "commandReductionRatio="
        f"{float(delta.get('commandReductionRatio') or 0):.3f}"
    )
    emit(
        "|readLoop "
        f"baselineDirectSourceReadCode={baseline.get('directSourceReadCodeCount', 0)} "
        f"candidateDirectSourceReadCode={candidate.get('directSourceReadCodeCount', 0)} "
        f"candidateDuplicateSelectors={candidate.get('duplicateSelectorCount', 0)} "
        f"candidateSameFileWindowFanout={candidate.get('sameFileWindowFanout', 0)}"
    )
    emit(
        "|bytes "
        f"baselineStdoutBytes={baseline.get('stdoutBytes', 0)} "
        f"candidateStdoutBytes={candidate.get('stdoutBytes', 0)} "
        "stdoutBytesReductionRatio="
        f"{float(delta.get('stdoutBytesReductionRatio') or 0):.3f}"
    )
    missing = string_list(frontier.get("missingHotBlocks"))
    covered = string_list(frontier.get("coveredHotBlocks"))
    emit(
        "|frontier "
        f"coveredHotBlocks={len(covered)} "
        f"expectedHotBlocks={len(string_list(frontier.get('expectedHotBlocks')))} "
        f"missingHotBlocks={len(missing)} "
        "declaredFailureFrontier="
        f"{len(list_value(frontier.get('declaredFailureFrontier')))}"
    )
    for target in missing:
        emit(f"|missingHotBlock target={quote_value(target)}")
    for failure in string_list(comparison.get("failures")):
        emit(f"|failure {failure}")


def _resolve_receipt_path(
    repo_root: Path,
    path: Path,
    *,
    path_base: Path | None = None,
) -> Path:
    if path.is_absolute():
        return path
    if path_base is not None:
        scenario_local = path_base / path
        if scenario_local.exists():
            return scenario_local
    return repo_root / path


def _receipt_metrics(receipt: dict[str, Any]) -> dict[str, Any]:
    commands = [
        command
        for command in list_value(receipt.get("commands"))
        if isinstance(command, dict)
    ]
    selectors = [
        _selector(command)
        for command in commands
        if _is_direct_source_read_code(command)
    ]
    selectors = [selector for selector in selectors if selector]
    duplicate_selector_count = _duplicate_count(selectors)
    return {
        "scenarioId": require_str(receipt, "scenarioId", "unknown"),
        "language": require_str(receipt, "language", "unknown"),
        "commandCount": _summary_int(receipt, "commandCount", len(commands)),
        "stdoutBytes": _summary_int(
            receipt,
            "stdoutBytes",
            sum(_command_metric(command, "stdoutBytes") for command in commands),
        ),
        "stderrBytes": _summary_int(
            receipt,
            "stderrBytes",
            sum(_command_metric(command, "stderrBytes") for command in commands),
        ),
        "directSourceReadCodeCount": len(selectors),
        "queryCodeCount": sum(1 for command in commands if _is_query_code(command)),
        "duplicateSelectorCount": duplicate_selector_count,
        "sameFileWindowFanout": _same_file_window_fanout(selectors),
        "selectors": selectors,
    }


def _summary_int(receipt: dict[str, Any], field: str, default: int) -> int:
    return optional_int(dict_value(receipt.get("summary")).get(field)) or default


def _command_metric(command: dict[str, Any], field: str) -> int:
    return optional_int(dict_value(command.get("metrics")).get(field)) or 0


def _is_direct_source_read_code(command: dict[str, Any]) -> bool:
    argv = _argv(command)
    return (
        _is_query_code(command)
        and _option_value(argv, "--from-hook") == "direct-source-read"
    )


def _is_query_code(command: dict[str, Any]) -> bool:
    argv = _argv(command)
    return "query" in argv and "--code" in argv


def _selector(command: dict[str, Any]) -> str:
    return _option_value(_argv(command), "--selector")


def _argv(command: dict[str, Any]) -> list[str]:
    return string_list(command.get("argv"))


def _option_value(argv: list[str], option: str) -> str:
    try:
        index = argv.index(option)
    except ValueError:
        return ""
    value_index = index + 1
    if value_index >= len(argv):
        return ""
    value = argv[value_index]
    return "" if value.startswith("-") else value


def _duplicate_count(values: list[str]) -> int:
    seen: set[str] = set()
    duplicates = 0
    for value in values:
        if value in seen:
            duplicates += 1
        else:
            seen.add(value)
    return duplicates


def _same_file_window_fanout(selectors: list[str]) -> int:
    counts: dict[str, int] = {}
    for selector in selectors:
        key = _selector_file(selector)
        counts[key] = counts.get(key, 0) + 1
    return sum(count - 1 for count in counts.values() if count > 1)


def _selector_file(selector: str) -> str:
    if ":" not in selector:
        return selector
    head, tail = selector.rsplit(":", 1)
    return head if tail.replace("-", "").isdigit() else selector


def _declared_failure_frontier(receipt: dict[str, Any]) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    for command in list_value(receipt.get("commands")):
        if not isinstance(command, dict):
            continue
        if not _is_check_command(command):
            continue
        for item in list_value(command.get("failureFrontier")):
            if isinstance(item, dict):
                entries.append(dict(item))
    return entries


def _declared_hot_blocks(
    receipt: dict[str, Any],
    *,
    declared_failure_frontier: list[dict[str, Any]],
) -> list[str]:
    targets: list[str] = []
    seen: set[str] = set()
    for entry in declared_failure_frontier:
        for field in ("hotBlockSelector", "nextSelector"):
            target = require_str(entry, field, "")
            if target and target not in seen:
                targets.append(target)
                seen.add(target)
    for command in list_value(receipt.get("commands")):
        if not isinstance(command, dict):
            continue
        if not _is_check_command(command):
            continue
        for target in string_list(command.get("next")):
            if target and target not in seen:
                targets.append(target)
                seen.add(target)
    return targets


def _read_targets(receipt: dict[str, Any]) -> set[str]:
    targets: set[str] = set()
    for command in list_value(receipt.get("commands")):
        if not isinstance(command, dict):
            continue
        if not _is_direct_source_read_code(command):
            continue
        selector = _selector(command)
        if selector:
            targets.add(selector)
    return targets


def _is_check_command(command: dict[str, Any]) -> bool:
    return command.get("kind") == "check" or "check" in _argv(command)


def _target_covered(target: str, frontier_targets: set[str]) -> bool:
    if target in frontier_targets:
        return True
    target_file = _selector_file(target)
    if target_file != target:
        return False
    return any(
        _selector_file(candidate) == target_file for candidate in frontier_targets
    )


def _coverage_ratio(covered: list[str], expected: list[str]) -> float | None:
    if not expected:
        return None
    return round(len(covered) / len(expected), 3)


def _ratio_reduction(baseline: int, candidate: int) -> float:
    if baseline <= 0:
        return 0.0
    return round((baseline - candidate) / baseline, 3)


def _comparison_failures(
    *,
    command_reduction_ratio: float,
    candidate_metrics: dict[str, Any],
    missing_hot_blocks: list[str],
    thresholds: FailureFrontierThresholds,
) -> list[str]:
    failures: list[str] = []
    if command_reduction_ratio < thresholds.min_command_reduction:
        failures.append(
            "commandReductionRatio="
            f"{command_reduction_ratio:.3f}<"
            f"{thresholds.min_command_reduction:.3f}"
        )
    direct_count = int(candidate_metrics["directSourceReadCodeCount"])
    if direct_count > thresholds.max_direct_source_read_code:
        failures.append(
            "directSourceReadCode="
            f"{direct_count}>{thresholds.max_direct_source_read_code}"
        )
    duplicate_count = int(candidate_metrics["duplicateSelectorCount"])
    if duplicate_count > thresholds.max_duplicate_selectors:
        failures.append(
            f"duplicateSelectors={duplicate_count}>{thresholds.max_duplicate_selectors}"
        )
    same_file_fanout = int(candidate_metrics["sameFileWindowFanout"])
    if same_file_fanout > thresholds.max_same_file_window_fanout:
        failures.append(
            "sameFileWindowFanout="
            f"{same_file_fanout}>{thresholds.max_same_file_window_fanout}"
        )
    if missing_hot_blocks:
        failures.append(f"missingHotBlocks={len(missing_hot_blocks)}")
    return failures
