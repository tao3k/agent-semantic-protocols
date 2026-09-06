# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Resolve stdin inputs for sandtable steps."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

from .models import StepResult
from .step_errors import empty_step_error
from .utils import count_lines, expand_string_list, expand_tokens, require_str


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
    return _stdin_command_output(
        step, stdin_command, workdir, scenario_id, env, captures
    )


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
