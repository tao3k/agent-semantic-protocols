"""Scenario evidence adapter for failure-frontier comparisons."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .failure_frontier_eval import (
    FailureFrontierThresholds,
    compare_failure_frontier_receipt_paths,
    compare_failure_frontier_receipts,
)
from .trace_receipt_events import TraceCommandFilter
from .trace_receipts import TraceReceiptConfig, build_receipt_from_trace_path
from .utils import dict_value, optional_int, require_str, string_list


def failure_frontier_comparison_from_evidence(
    repo_root: Path,
    *,
    path_base: Path | None = None,
    scenario_id: str,
    language: str,
    evidence: dict[str, Any],
) -> dict[str, Any] | None:
    config = dict_value(evidence.get("failureFrontierComparison"))
    if not config:
        return None
    expected_hot_blocks = string_list(config.get("expectedHotBlocks"))
    thresholds = failure_frontier_thresholds(config.get("thresholds"))
    if _has_trace_pair(config):
        return compare_failure_frontier_receipts(
            _trace_receipt(
                repo_root,
                config,
                scenario_id,
                language,
                "baseline",
                path_base=path_base,
            ),
            _trace_receipt(
                repo_root,
                config,
                scenario_id,
                language,
                "candidate",
                path_base=path_base,
            ),
            expected_hot_blocks=expected_hot_blocks,
            thresholds=thresholds,
        )
    return compare_failure_frontier_receipt_paths(
        repo_root,
        Path(require_str(config, "baselineReceiptPath", "")),
        Path(require_str(config, "candidateReceiptPath", "")),
        path_base=path_base,
        expected_hot_blocks=expected_hot_blocks,
        thresholds=thresholds,
    )


def failure_frontier_error(comparison: dict[str, Any]) -> str:
    failures = string_list(comparison.get("failures"))
    return (
        "failure-frontier comparison failed"
        + (f": {', '.join(failures)}" if failures else "")
    )


def failure_frontier_thresholds(value: Any) -> FailureFrontierThresholds:
    thresholds = dict_value(value)
    defaults = FailureFrontierThresholds()
    return FailureFrontierThresholds(
        min_command_reduction=_optional_float(
            thresholds.get("minCommandReduction"),
            defaults.min_command_reduction,
        ),
        max_direct_source_read_code=_optional_threshold_int(
            thresholds.get("maxDirectSourceReadCode"),
            defaults.max_direct_source_read_code,
        ),
        max_duplicate_selectors=_optional_threshold_int(
            thresholds.get("maxDuplicateSelectors"),
            defaults.max_duplicate_selectors,
        ),
        max_same_file_window_fanout=_optional_threshold_int(
            thresholds.get("maxSameFileWindowFanout"),
            defaults.max_same_file_window_fanout,
        ),
    )


def _trace_receipt(
    repo_root: Path,
    config: dict[str, Any],
    scenario_id: str,
    language: str,
    side: str,
    *,
    path_base: Path | None = None,
) -> dict[str, Any]:
    trace_path = _trace_path(repo_root, config, side, path_base=path_base)
    return build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id=f"{scenario_id}.{side}",
            language=language,
            project_name=require_str(config, "projectName", "unknown"),
            project_source=require_str(config, "projectSource", "fixture"),
            intent=require_str(config, "intent", "Compare failure-frontier traces."),
            edit_boundary=require_str(config, "editBoundary", "before-edit"),
            recorded_at=_optional_str(config.get("recordedAt")),
        ),
        filters=_trace_filter(config, side),
    )


def _trace_path(
    repo_root: Path,
    config: dict[str, Any],
    side: str,
    *,
    path_base: Path | None = None,
) -> Path:
    field = f"{side}TracePath"
    path = Path(require_str(config, field, ""))
    if path.is_absolute():
        return path
    if path_base is not None:
        scenario_local = path_base / path
        if scenario_local.exists():
            return scenario_local
    return repo_root / path


def _has_trace_pair(config: dict[str, Any]) -> bool:
    return isinstance(config.get("baselineTracePath"), str) or isinstance(
        config.get("candidateTracePath"),
        str,
    )


def _trace_filter(config: dict[str, Any], side: str) -> TraceCommandFilter:
    return TraceCommandFilter(
        session_id=_optional_str(config.get(f"{side}TraceSessionId")),
        language_id=_optional_str(config.get("traceLanguageId")),
        provider_id=_optional_str(config.get("traceProviderId")),
    )


def _optional_float(value: Any, default: float) -> float:
    if value is None:
        return default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _optional_threshold_int(value: Any, default: int) -> int:
    parsed = optional_int(value)
    return parsed if parsed is not None else default


def _optional_str(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None
