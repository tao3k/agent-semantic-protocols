"""Execute individual sandtable steps."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

from .agent_observations import summarize_agent_stdout
from .expectations import capture_values, validate_step
from .models import StepResult
from .step_agent_cli import resolve_agent_cli_step as _resolve_agent_cli_step
from .step_agent_sdk import resolve_agent_sdk_step as _resolve_agent_sdk_step
from .step_errors import empty_step_error
from .step_process import (
    completed_step_result as _completed_step_result,
    resolve_step_command as _resolve_step_command,
    run_step_process as _run_step_process,
    workspace_dev_command as _workspace_dev_command,
)
from .step_stdin import resolve_stdin
from .utils import require_str


def run_step(
    *,
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    step: Any,
    index: int,
    env: dict[str, str],
    captures: dict[str, str],
) -> StepResult:
    if not isinstance(step, dict):
        return empty_step_error(scenario_id, f"step-{index}", "step must be an object")

    step_id = require_str(step, "id", f"step-{index}")
    return _run_valid_step(
        repo_root=repo_root,
        workdir=workdir,
        scenario_id=scenario_id,
        step=step,
        step_id=step_id,
        env=env,
        captures=captures,
    )


def _run_valid_step(
    *,
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    step: dict[str, Any],
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> StepResult:
    execution = _resolve_step_execution(
        step,
        scenario_id,
        step_id,
        env,
        captures,
        repo_root=repo_root,
    )
    if isinstance(execution, StepResult):
        return execution
    command, step_env = execution
    command = _workspace_dev_command(
        repo_root,
        command,
        benchmark_binary=step_env.get("ASP_BENCHMARK_BIN"),
    )
    stdin = resolve_stdin(step, workdir, scenario_id, step_env, captures)
    if isinstance(stdin, StepResult):
        return stdin

    timeout_seconds = float(step.get("timeoutSeconds", 30))
    process = _run_step_process(
        command,
        workdir,
        step_env,
        stdin,
        timeout_seconds,
        scenario_id,
        step_id,
    )
    if isinstance(process, StepResult):
        return process

    completed_process, elapsed_ms = process
    result = _completed_step_result(
        scenario_id,
        step_id,
        command,
        completed_process,
        elapsed_ms,
    )
    _observe_agent_step(step, result, completed_process.stdout)
    _validate_and_capture_step(step, result, completed_process, repo_root, captures)
    return result


def _resolve_step_execution(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
    repo_root: Path | None = None,
) -> tuple[list[str], dict[str, str]] | StepResult:
    has_command = "command" in step
    has_agent_cli = "agentCli" in step
    has_agent_sdk = "agentSdk" in step
    if sum([has_command, has_agent_cli, has_agent_sdk]) != 1:
        return empty_step_error(
            scenario_id,
            step_id,
            "step must define exactly one of command, agentCli, or agentSdk",
        )
    if has_agent_cli:
        return _resolve_agent_cli_step(step, scenario_id, step_id, env, captures)
    if has_agent_sdk:
        return _resolve_agent_sdk_step(
            step,
            scenario_id,
            step_id,
            env,
            captures,
            repo_root,
        )

    command = _resolve_step_command(step, scenario_id, step_id, captures)
    if isinstance(command, StepResult):
        return command
    return command, env


def _validate_and_capture_step(
    step: dict[str, Any],
    result: StepResult,
    process: subprocess.CompletedProcess[str],
    repo_root: Path,
    captures: dict[str, str],
) -> None:
    validate_step(step, result, process.stdout, process.stderr, repo_root)
    capture_values(step, result, process.stdout, captures)
    if result.errors:
        result.status = "fail"
    elif result.warnings:
        result.status = "warn"


def _observe_agent_step(step: dict[str, Any], result: StepResult, stdout: str) -> None:
    expect = step.get("expect", {})
    expects_pipe_flow = isinstance(expect, dict) and isinstance(
        expect.get("pipeFlow"), dict
    )
    if "agentCli" not in step and "agentSdk" not in step and not expects_pipe_flow:
        return
    observations = summarize_agent_stdout(stdout)
    if observations:
        result.observations = observations
