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
    list_value,
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
    warmup_steps = _resolve_warmup_steps(scenario, result)
    if warmup_steps is None:
        return result
    _run_scenario_warmup(
        repo_root,
        workdir,
        scenario_id,
        warmup_steps,
        env,
        captures,
        result,
    )
    if result.status == "fail":
        return result
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


def _resolve_warmup_steps(
    scenario: dict[str, Any],
    result: ScenarioResult,
) -> list[Any] | None:
    warmup = scenario.get("warmup", [])
    if not isinstance(warmup, list):
        result.status = "fail"
        result.errors.append("scenario.warmup must be an array")
        return None
    return warmup


def _run_scenario_warmup(
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    warmup_steps: list[Any],
    env: dict[str, str],
    captures: dict[str, str],
    result: ScenarioResult,
) -> None:
    if not warmup_steps:
        return
    summaries: list[dict[str, Any]] = []
    for index, step in enumerate(warmup_steps, start=1):
        step_result = _run_step_with_cold_retry(
            repo_root=repo_root,
            workdir=workdir,
            scenario_id=scenario_id,
            step=step,
            index=index,
            env=env,
            captures=captures,
        )
        summaries.append(
            {
                "id": step_result.step_id,
                "status": step_result.status,
                "elapsedMs": step_result.elapsed_ms,
            }
        )
        if step_result.status == "fail":
            result.status = "fail"
            result.errors.append(f"warmup {step_result.step_id} failed")
            result.errors.extend(step_result.errors)
            break
    result.evidence["warmup"] = {"steps": summaries}


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
    steps = _resolve_scenario_steps(scenario, result)
    if steps is None:
        return result
    execution = dict_value(scenario.get("execution"))
    _warn_on_command_budget(result, steps, max_commands)
    return scenario_id, workdir, result, steps, budgets, execution


def _resolve_scenario_steps(
    scenario: dict[str, Any],
    result: ScenarioResult,
) -> list[Any] | None:
    steps = scenario.get("steps")
    if steps is not None:
        if not isinstance(steps, list):
            result.status = "fail"
            result.errors.append("scenario.steps must be an array")
            return None
        return steps

    live_agent = scenario.get("liveAgent")
    if live_agent is None:
        result.status = "fail"
        result.errors.append("scenario must define steps or liveAgent")
        return None
    if not isinstance(live_agent, dict):
        result.status = "fail"
        result.errors.append("scenario.liveAgent must be an object")
        return None
    return _live_agent_steps_from_deep_question(scenario, live_agent, result)


def _live_agent_steps_from_deep_question(
    scenario: dict[str, Any],
    live_agent: dict[str, Any],
    result: ScenarioResult,
) -> list[Any] | None:
    evidence = dict_value(scenario.get("evidence"))
    cases = list_value(evidence.get("deepQuestionCases"))
    if len(cases) != 1 or not isinstance(cases[0], dict):
        result.status = "fail"
        result.errors.append(
            "scenario.liveAgent requires exactly one evidence.deepQuestionCases item"
        )
        return None

    question_case = cases[0]
    prompt = require_str(question_case, "question", "")
    if not prompt:
        result.status = "fail"
        result.errors.append("scenario.liveAgent deep question must have question")
        return None

    step_ids = string_list(question_case.get("stepIds"))
    step_id = step_ids[0] if step_ids else require_str(question_case, "id", "prompt")
    expect = dict_value(live_agent.get("expect"))
    require_asp_bash_commands = bool(
        live_agent.get("requireAspBashCommands", True)
    )
    agent_sdk = {
        "client": require_str(live_agent, "client", "claude"),
        "prompt": prompt,
        "outputFormat": require_str(live_agent, "outputFormat", "stream-json"),
        "includeHookEvents": bool(live_agent.get("includeHookEvents", True)),
        "verbose": bool(live_agent.get("verbose", True)),
        "requireAspBashCommands": require_asp_bash_commands,
        "useRepoClaudeSettings": True,
        "env": dict_value(
            live_agent.get("env", {"ANTHROPIC_AUTH_TOKEN": "${ANTHROPIC_AUTH_TOKEN}"})
        ),
        "requiredEnv": string_list(
            live_agent.get("requiredEnv", ["ANTHROPIC_AUTH_TOKEN"])
        ),
    }
    allowed_tools = string_list(live_agent.get("allowedTools"))
    if allowed_tools:
        agent_sdk["allowedTools"] = allowed_tools
    if isinstance(live_agent.get("model"), str):
        agent_sdk["model"] = live_agent["model"]
    pipe_expect = dict_value(expect.get("pipeFlow"))
    max_asp_commands = optional_int(pipe_expect.get("maxAspCommands"))
    if max_asp_commands is not None:
        agent_sdk["maxAspBashCommands"] = max_asp_commands

    step: dict[str, Any] = {
        "id": step_id,
        "kind": "agent-sdk",
        "agentSdk": agent_sdk,
    }
    timeout_seconds = live_agent.get("timeoutSeconds")
    if timeout_seconds is not None:
        step["timeoutSeconds"] = timeout_seconds
    if expect:
        step["expect"] = expect
    return [step]


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
    receipt_error = validate_linked_receipt(
        repo_root, result.evidence, path_base=result.path.parent
    )
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
        path_base=result.path.parent,
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
        step_result = _run_step_with_cold_retry(
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


def _max_elapsed_error(elapsed_ms: int, maximum: int) -> str:
    return f"elapsedMs={elapsed_ms} exceeds maxElapsedMs={maximum}"


def _cold_start_elapsed_error(elapsed_ms: int, maximum: int) -> str:
    return f"elapsedMs={elapsed_ms} exceeds maxColdStartElapsedMs={maximum}"


def _cold_retry_limits(step: Any) -> tuple[int, int] | None:
    if not isinstance(step, dict):
        return None
    expect = step.get("expect")
    if not isinstance(expect, dict):
        return None
    maximum = optional_int(expect.get("maxElapsedMs"))
    cold_maximum = optional_int(expect.get("maxColdStartElapsedMs"))
    if maximum is None or cold_maximum is None:
        return None
    return maximum, cold_maximum


def _should_retry_cold_start(step: Any, step_result: StepResult) -> tuple[int, int] | None:
    limits = _cold_retry_limits(step)
    if limits is None:
        return None
    maximum, cold_maximum = limits
    warm_error = _max_elapsed_error(step_result.elapsed_ms, maximum)
    cold_error = _cold_start_elapsed_error(step_result.elapsed_ms, cold_maximum)
    if (
        step_result.status == "fail"
        and step_result.errors == [warm_error]
        and cold_error not in step_result.errors
        and maximum < step_result.elapsed_ms <= cold_maximum
    ):
        return limits
    return None


def _run_step_with_cold_retry(
    *,
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    step: Any,
    index: int,
    env: dict[str, str],
    captures: dict[str, str],
) -> StepResult:
    first_result = run_step(
        repo_root=repo_root,
        workdir=workdir,
        scenario_id=scenario_id,
        step=step,
        index=index,
        env=env,
        captures=captures,
    )
    retry_limits = _should_retry_cold_start(step, first_result)
    if retry_limits is None:
        return first_result

    maximum, cold_maximum = retry_limits
    retry_result = run_step(
        repo_root=repo_root,
        workdir=workdir,
        scenario_id=scenario_id,
        step=step,
        index=index,
        env=env,
        captures=captures,
    )
    retry_result.observations["coldStartRetry"] = {
        "firstElapsedMs": first_result.elapsed_ms,
        "finalElapsedMs": retry_result.elapsed_ms,
        "maxElapsedMs": maximum,
        "maxColdStartElapsedMs": cold_maximum,
        "firstErrors": list(first_result.errors),
    }
    return retry_result


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
