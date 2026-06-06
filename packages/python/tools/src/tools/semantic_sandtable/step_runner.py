"""Execute individual sandtable steps."""

from __future__ import annotations

import os
import re
import subprocess
import time
from pathlib import Path
from typing import Any

from .expectations import capture_values, validate_step
from .models import StepResult
from .utils import (
    count_lines,
    expand_string_list,
    expand_tokens,
    require_str,
    string_list,
)


_ENV_REFERENCE_PATTERN = re.compile(
    r"\$(?:\{[A-Za-z_][A-Za-z0-9_]*\}|[A-Za-z_][A-Za-z0-9_]*)"
)


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
    execution = _resolve_step_execution(step, scenario_id, step_id, env, captures)
    if isinstance(execution, StepResult):
        return execution
    command, step_env = execution
    command = _workspace_dev_command(repo_root, command)
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
    _validate_and_capture_step(step, result, completed_process, repo_root, captures)
    return result


def _resolve_step_execution(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> tuple[list[str], dict[str, str]] | StepResult:
    has_command = "command" in step
    has_agent_cli = "agentCli" in step
    if has_command and has_agent_cli:
        return empty_step_error(
            scenario_id,
            step_id,
            "step must define exactly one of command or agentCli",
        )
    if has_agent_cli:
        return _resolve_agent_cli_step(step, scenario_id, step_id, env, captures)

    command = _resolve_step_command(step, scenario_id, step_id, captures)
    if isinstance(command, StepResult):
        return command
    return command, env


def _resolve_agent_cli_step(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> tuple[list[str], dict[str, str]] | StepResult:
    spec = step.get("agentCli")
    if not isinstance(spec, dict):
        return empty_step_error(scenario_id, step_id, "step.agentCli must be an object")
    if require_str(spec, "client", "") != "claude":
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentCli.client must be 'claude'",
        )

    resolved = _resolve_claude_cli_command(spec, scenario_id, step_id, captures)
    if isinstance(resolved, StepResult):
        return resolved
    step_env = _resolve_agent_cli_env(spec, scenario_id, step_id, env)
    if isinstance(step_env, StepResult):
        return step_env
    return resolved, step_env


def _resolve_claude_cli_command(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    binary = _required_agent_cli_string(spec, "binary", scenario_id, step_id, captures)
    prompt = _required_agent_cli_string(spec, "prompt", scenario_id, step_id, captures)
    output_format = _required_agent_cli_string(
        spec,
        "outputFormat",
        scenario_id,
        step_id,
        captures,
    )
    if isinstance(binary, StepResult):
        return binary
    if isinstance(prompt, StepResult):
        return prompt
    if isinstance(output_format, StepResult):
        return output_format
    if output_format not in {"text", "json", "stream-json"}:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentCli.outputFormat must be text, json, or stream-json",
        )

    command = [binary, "-p", prompt, "--output-format", output_format]
    input_format = _optional_agent_cli_string(
        spec,
        "inputFormat",
        scenario_id,
        step_id,
        captures,
    )
    if isinstance(input_format, StepResult):
        return input_format
    if input_format:
        if input_format not in {"text", "stream-json"}:
            return empty_step_error(
                scenario_id,
                step_id,
                "step.agentCli.inputFormat must be text or stream-json",
            )
        command.extend(["--input-format", input_format])
    if bool(spec.get("includePartialMessages", False)):
        command.append("--include-partial-messages")
    if bool(spec.get("includeHookEvents", False)):
        command.append("--include-hook-events")
    if bool(spec.get("verbose", False)):
        command.append("--verbose")

    model = _optional_agent_cli_string(spec, "model", scenario_id, step_id, captures)
    if isinstance(model, StepResult):
        return model
    if model:
        command.extend(["--model", model])
    return command


def _required_agent_cli_string(
    spec: dict[str, Any],
    key: str,
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> str | StepResult:
    value = spec.get(key)
    if not isinstance(value, str) or not value:
        return empty_step_error(
            scenario_id,
            step_id,
            f"step.agentCli.{key} must be a non-empty string",
        )
    try:
        return expand_tokens(value, captures)
    except KeyError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"missing capture {error.args[0]!r}",
        )


def _optional_agent_cli_string(
    spec: dict[str, Any],
    key: str,
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> str | StepResult | None:
    value = spec.get(key)
    if value is None:
        return None
    if not isinstance(value, str):
        return empty_step_error(
            scenario_id,
            step_id,
            f"step.agentCli.{key} must be a string",
        )
    try:
        return expand_tokens(value, captures)
    except KeyError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"missing capture {error.args[0]!r}",
        )


def _resolve_agent_cli_env(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
) -> dict[str, str] | StepResult:
    step_env = env.copy()
    overrides = spec.get("env", {})
    if not isinstance(overrides, dict):
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentCli.env must be an object",
        )
    for key, value in overrides.items():
        if not isinstance(key, str) or not isinstance(value, str):
            return empty_step_error(
                scenario_id,
                step_id,
                "step.agentCli.env entries must be string to string",
            )
        step_env[key] = _expand_env_references(value, step_env)

    missing = [
        name
        for name in string_list(spec.get("requiredEnv"))
        if _missing_required_env_value(step_env.get(name))
    ]
    if missing:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentCli.requiredEnv unresolved: " + ", ".join(missing),
        )
    return step_env


def _expand_env_references(value: str, env: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        token = match.group(0)
        name = token[2:-1] if token.startswith("${") else token[1:]
        return env.get(name, os.environ.get(name, token))

    return _ENV_REFERENCE_PATTERN.sub(replace, value)


def _missing_required_env_value(value: str | None) -> bool:
    return not value or _ENV_REFERENCE_PATTERN.search(value) is not None


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
    if command[0] != "asp":
        return command
    rewritten = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(repo_root / "crates" / "agent-semantic-protocol" / "Cargo.toml"),
        "--",
        *command[1:],
    ]
    if _is_hook_command(command) and "--activation" not in command:
        rewritten.extend(
            [
                "--activation",
                str(
                    repo_root
                    / ".cache"
                    / "agent-semantic-protocol"
                    / "hooks"
                    / "activation.json"
                ),
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
