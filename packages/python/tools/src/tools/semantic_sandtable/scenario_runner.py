"""Execute semantic sandtable scenarios."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any

from .budgets import warn_scenario_if_over
from .failure_frontier_scenario import (
    failure_frontier_comparison_from_evidence,
    failure_frontier_error,
)
from .models import ScenarioLoadError, ScenarioResult, StepResult, has_warnings
from .receipts import validate_linked_receipt
from .scenario_io import load_scenario
from .step_runner import run_step
from .utils import (
    build_env,
    dict_value,
    optional_int,
    require_str,
    resolve_workdir_with_env,
    string_list,
)


def run_scenario(repo_root: Path, path: Path) -> ScenarioResult:
    scenario = _load_scenario_or_result(repo_root, path)
    if isinstance(scenario, ScenarioResult):
        return scenario
    return _run_loaded_scenario(repo_root, path, scenario)


def _run_loaded_scenario(
    repo_root: Path,
    path: Path,
    scenario: dict[str, Any],
) -> ScenarioResult:
    env = build_env(scenario.get("env", {}), repo_root=repo_root)
    prepared = _prepare_loaded_scenario(repo_root, path, scenario, env)
    if isinstance(prepared, ScenarioResult):
        return prepared
    scenario_id, workdir, result, steps, budgets, execution = prepared
    captures: dict[str, str] = {}
    totals = _run_scenario_steps(
        repo_root,
        workdir,
        scenario_id,
        steps,
        env,
        captures,
        result,
        execution,
    )
    _apply_scenario_budget_warnings(result, budgets, totals)
    _finalize_scenario_status(result)
    return result


def _prepare_loaded_scenario(
    repo_root: Path,
    path: Path,
    scenario: dict[str, Any],
    env: dict[str, str],
) -> (
    tuple[str, Path, ScenarioResult, list[Any], dict[str, Any], dict[str, Any]]
    | ScenarioResult
):
    scenario_id = require_str(scenario, "id", path.stem)
    language = require_str(scenario, "language", "unknown")
    workdir = resolve_workdir_with_env(repo_root, scenario.get("workdir"), env)
    result = _initial_scenario_result(scenario, scenario_id, language, path, workdir)
    if _apply_receipt_error(repo_root, result):
        return result
    if _apply_failure_frontier_comparison(repo_root, result):
        return result
    if _apply_env_gate_skip(result, scenario, env):
        return result
    if workdir is None:
        result.status = "skip"
        result.skip_reason = "no workdir candidate exists"
        return result

    budgets = scenario.get("budgets", {})
    max_commands = optional_int(budgets.get("maxCommands"))
    steps = scenario.get("steps", [])
    if not isinstance(steps, list):
        result.status = "fail"
        result.errors.append("scenario.steps must be an array")
        return result
    execution = dict_value(scenario.get("execution"))
    _warn_on_command_budget(result, steps, max_commands)
    return scenario_id, workdir, result, steps, budgets, execution


def _finalize_scenario_status(result: ScenarioResult) -> None:
    if result.status != "fail" and has_warnings(result):
        result.status = "warn"


def _load_scenario_or_result(
    repo_root: Path,
    path: Path,
) -> dict[str, Any] | ScenarioResult:
    try:
        return load_scenario(path, repo_root)
    except ScenarioLoadError as error:
        return ScenarioResult(
            scenario_id=path.stem,
            language="unknown",
            path=path,
            status="fail",
            workdir=None,
            errors=[str(error)],
        )


def _initial_scenario_result(
    scenario: dict[str, Any],
    scenario_id: str,
    language: str,
    path: Path,
    workdir: Path | None,
) -> ScenarioResult:
    return ScenarioResult(
        scenario_id=scenario_id,
        language=language,
        path=path,
        status="pass",
        workdir=workdir,
        coverage=string_list(scenario.get("coverage", [])),
        tags=string_list(scenario.get("tags", [])),
        evidence=dict_value(scenario.get("evidence")),
        workdir_spec=scenario.get("workdir"),
    )


def _apply_env_gate_skip(
    result: ScenarioResult, scenario: dict[str, Any], env: dict[str, str]
) -> bool:
    missing = [
        name for name in string_list(scenario.get("skipUnlessEnv")) if not env.get(name)
    ]
    if not missing:
        return False
    result.status = "skip"
    result.skip_reason = "missing env: " + ",".join(missing)
    return True


def _apply_receipt_error(repo_root: Path, result: ScenarioResult) -> bool:
    receipt_error = validate_linked_receipt(repo_root, result.evidence)
    if receipt_error is None:
        return False
    result.status = "fail"
    result.errors.append(receipt_error)
    return True


def _apply_failure_frontier_comparison(
    repo_root: Path,
    result: ScenarioResult,
) -> bool:
    config = dict_value(result.evidence.get("failureFrontierComparison"))
    if not config:
        return False
    comparison = failure_frontier_comparison_from_evidence(
        repo_root,
        scenario_id=result.scenario_id,
        language=result.language,
        evidence=result.evidence,
    )
    if comparison is None:
        return False
    result.evidence["failureFrontierComparisonResult"] = comparison
    if comparison["status"] == "pass":
        return False
    result.status = "fail"
    result.errors.append(failure_frontier_error(comparison))
    return True


def _warn_on_command_budget(
    result: ScenarioResult,
    steps: list[Any],
    max_commands: int | None,
) -> None:
    if max_commands is not None and len(steps) > max_commands:
        result.warnings.append(
            f"commands={len(steps)} exceeds maxCommands={max_commands}"
        )


def _run_scenario_steps(
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    steps: list[Any],
    env: dict[str, str],
    captures: dict[str, str],
    result: ScenarioResult,
    execution: dict[str, Any] | None = None,
) -> dict[str, int]:
    totals = {"lines": 0, "elapsedMs": 0, "stdoutBytes": 0, "stderrBytes": 0}
    max_concurrent_steps = _max_concurrent_steps(execution)
    if max_concurrent_steps > 1:
        return _run_scenario_steps_parallel(
            repo_root,
            workdir,
            scenario_id,
            steps,
            env,
            captures,
            result,
            totals,
            max_concurrent_steps,
        )
    for index, step in enumerate(steps, start=1):
        step_result = run_step(
            repo_root=repo_root,
            workdir=workdir,
            scenario_id=scenario_id,
            step=step,
            index=index,
            env=env,
            captures=captures,
        )
        _record_step_result(result, step_result, totals)
    return totals


def _run_scenario_steps_parallel(
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    steps: list[Any],
    env: dict[str, str],
    captures: dict[str, str],
    result: ScenarioResult,
    totals: dict[str, int],
    max_concurrent_steps: int,
) -> dict[str, int]:
    indexed_results: list[tuple[int, StepResult]] = []
    with ThreadPoolExecutor(max_workers=max_concurrent_steps) as executor:
        futures = {
            executor.submit(
                run_step,
                repo_root=repo_root,
                workdir=workdir,
                scenario_id=scenario_id,
                step=step,
                index=index,
                env=env,
                captures=captures.copy(),
            ): index
            for index, step in enumerate(steps, start=1)
        }
        for future in as_completed(futures):
            indexed_results.append((futures[future], future.result()))
    for _index, step_result in sorted(indexed_results, key=lambda item: item[0]):
        _record_step_result(result, step_result, totals)
    return totals


def _max_concurrent_steps(execution: dict[str, Any] | None) -> int:
    if not execution:
        return 1
    mode = execution.get("mode")
    raw_value = optional_int(execution.get("maxConcurrentSteps"))
    if mode == "sequential":
        return 1
    return max(1, raw_value or 1)


def _record_step_result(
    result: ScenarioResult,
    step_result: StepResult,
    totals: dict[str, int],
) -> None:
    result.steps.append(step_result)
    totals["lines"] += step_result.stdout_lines + step_result.stderr_lines
    totals["elapsedMs"] += step_result.elapsed_ms
    totals["stdoutBytes"] += step_result.stdout_bytes
    totals["stderrBytes"] += step_result.stderr_bytes
    if step_result.status == "fail":
        result.status = "fail"


def _apply_scenario_budget_warnings(
    result: ScenarioResult,
    budgets: dict[str, Any],
    totals: dict[str, int],
) -> None:
    max_total_lines_warn = optional_int(budgets.get("maxTotalLinesWarn"))
    if max_total_lines_warn is not None and totals["lines"] > max_total_lines_warn:
        result.warnings.append(
            f"totalLines={totals['lines']} exceeds maxTotalLinesWarn={max_total_lines_warn}"
        )
    warn_scenario_if_over(
        result,
        "totalElapsedMs",
        totals["elapsedMs"],
        "maxTotalElapsedMsWarn",
        budgets.get("maxTotalElapsedMsWarn"),
    )
    warn_scenario_if_over(
        result,
        "totalStdoutBytes",
        totals["stdoutBytes"],
        "maxTotalStdoutBytesWarn",
        budgets.get("maxTotalStdoutBytesWarn"),
    )
    warn_scenario_if_over(
        result,
        "totalStderrBytes",
        totals["stderrBytes"],
        "maxTotalStderrBytesWarn",
        budgets.get("maxTotalStderrBytesWarn"),
    )
