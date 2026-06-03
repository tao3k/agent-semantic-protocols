"""Execute individual sandtable steps."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path
from typing import Any

from .expectations import capture_values, validate_step
from .models import StepResult
from .utils import count_lines, expand_string_list, expand_tokens, require_str


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
    command = _resolve_step_command(step, scenario_id, step_id, captures)
    if isinstance(command, StepResult):
        return command
    command = _workspace_dev_command(repo_root, command)
    stdin = resolve_stdin(step, workdir, scenario_id, env, captures)
    if isinstance(stdin, StepResult):
        return stdin

    timeout_seconds = float(step.get("timeoutSeconds", 30))
    process = _run_step_process(
        command,
        workdir,
        env,
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
    _validate_and_capture_step(step, result, completed_process, repo_root, captures)
    return result


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


def _resolve_step_command(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    command, command_errors = expand_string_list(step.get("command"), captures)
    if command_errors:
        return empty_step_error(scenario_id, step_id, "; ".join(command_errors))
    if not command:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.command must be a non-empty string array",
        )
    return command


def _workspace_dev_command(repo_root: Path, command: list[str]) -> list[str]:
    if command[0] != "semantic-agent-hook":
        return command
    rewritten = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(repo_root / "crates" / "semantic-agent-hook" / "Cargo.toml"),
        "--",
        *command[1:],
    ]
    if _is_hook_command(command) and "--activation" not in command:
        rewritten.extend(
            [
                "--activation",
                str(repo_root / ".codex" / "semantic-agent-hook" / "activation.json"),
            ]
        )
    return rewritten


def _is_hook_command(command: list[str]) -> bool:
    return len(command) > 1 and command[1] == "hook"


def _run_step_process(
    command: list[str],
    workdir: Path,
    env: dict[str, str],
    stdin: str | None,
    timeout_seconds: float,
    scenario_id: str,
    step_id: str,
) -> tuple[subprocess.CompletedProcess[str], int] | StepResult:
    started = time.perf_counter()
    try:
        process = subprocess.run(
            command,
            cwd=workdir,
            env=env,
            input=stdin,
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
    except FileNotFoundError as error:
        return _process_setup_error_result(
            scenario_id=scenario_id,
            step_id=step_id,
            command=command,
            started=started,
            error=f"command not found: {error.filename}",
        )
    except subprocess.TimeoutExpired:
        return _process_setup_error_result(
            scenario_id=scenario_id,
            step_id=step_id,
            command=command,
            started=started,
            error=f"timeout after {timeout_seconds:g}s",
        )
    return process, round((time.perf_counter() - started) * 1000)


def _process_setup_error_result(
    *,
    scenario_id: str,
    step_id: str,
    command: list[str],
    started: float,
    error: str,
) -> StepResult:
    return StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=command,
        status="fail",
        exit_code=None,
        elapsed_ms=round((time.perf_counter() - started) * 1000),
        stdout_lines=0,
        stderr_lines=0,
        stdout_bytes=0,
        stderr_bytes=0,
        errors=[error],
    )


def _completed_step_result(
    scenario_id: str,
    step_id: str,
    command: list[str],
    process: subprocess.CompletedProcess[str],
    elapsed_ms: int,
) -> StepResult:
    return StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=command,
        status="pass",
        exit_code=process.returncode,
        elapsed_ms=elapsed_ms,
        stdout_lines=count_lines(process.stdout),
        stderr_lines=count_lines(process.stderr),
        stdout_bytes=len(process.stdout.encode("utf-8")),
        stderr_bytes=len(process.stderr.encode("utf-8")),
    )


def empty_step_error(scenario_id: str, step_id: str, error: str) -> StepResult:
    return StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=[],
        status="fail",
        exit_code=None,
        elapsed_ms=0,
        stdout_lines=0,
        stderr_lines=0,
        stdout_bytes=0,
        stderr_bytes=0,
        errors=[error],
    )


def resolve_stdin(
    step: dict[str, Any],
    workdir: Path,
    scenario_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> str | None | StepResult:
    if "stdin" in step:
        return _literal_stdin(step, scenario_id, captures)

    stdin_command = step.get("stdinCommand")
    if stdin_command is None:
        return None
    return _stdin_command_output(step, stdin_command, workdir, scenario_id, env, captures)


def _literal_stdin(
    step: dict[str, Any],
    scenario_id: str,
    captures: dict[str, str],
) -> str | StepResult:
    value = step["stdin"]
    step_id = require_str(step, "id", "stdin")
    if not isinstance(value, str):
        return empty_step_error(scenario_id, step_id, "step.stdin must be a string")
    try:
        return expand_tokens(value, captures)
    except KeyError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"missing capture {error.args[0]!r}",
        )


def _stdin_command_output(
    step: dict[str, Any],
    stdin_command: Any,
    workdir: Path,
    scenario_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> str | StepResult:
    step_id = require_str(step, "id", "stdin-command")
    command, command_errors = expand_string_list(stdin_command, captures)
    if command_errors:
        return empty_step_error(scenario_id, step_id, "; ".join(command_errors))
    if not command:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.stdinCommand must be a non-empty string array",
        )
    return _run_stdin_command(step, command, workdir, scenario_id, step_id, env)


def _run_stdin_command(
    step: dict[str, Any],
    command: list[str],
    workdir: Path,
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
) -> str | StepResult:
    try:
        process = subprocess.run(
            command,
            cwd=workdir,
            env=env,
            text=True,
            capture_output=True,
            timeout=float(step.get("stdinTimeoutSeconds", 15)),
            check=False,
        )
    except FileNotFoundError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"stdin command not found: {error.filename}",
        )
    except subprocess.TimeoutExpired:
        return empty_step_error(scenario_id, step_id, "stdin command timeout")

    allow_non_zero = bool(step.get("stdinCommandAllowNonZero", False))
    if process.returncode != 0 and not allow_non_zero:
        return _stdin_command_error_result(scenario_id, step_id, command, process)
    return process.stdout


def _stdin_command_error_result(
    scenario_id: str,
    step_id: str,
    command: list[str],
    process: subprocess.CompletedProcess[str],
) -> StepResult:
    result = empty_step_error(
        scenario_id,
        step_id,
        f"stdin command exited {process.returncode}",
    )
    result.command = command
    result.exit_code = process.returncode
    result.stdout_lines = count_lines(process.stdout)
    result.stderr_lines = count_lines(process.stderr)
    result.stdout_bytes = len(process.stdout.encode("utf-8"))
    result.stderr_bytes = len(process.stderr.encode("utf-8"))
    return result
