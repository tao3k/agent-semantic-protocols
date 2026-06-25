"""Resolve and run sandtable step processes."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path
from typing import Any

from .agent_observations import summarize_agent_stdout
from .models import StepResult
from .step_errors import empty_step_error
from .utils import count_lines, expand_string_list


def resolve_step_command(
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


def workspace_dev_command(repo_root: Path, command: list[str]) -> list[str]:
    if command[0] == "ts-harness":
        if typescript_harness := _typescript_harness_dist_entry(repo_root):
            return ["node", str(typescript_harness), *command[1:]]
        return command
    if command[0] != "asp":
        return command
    if len(command) > 1 and command[1] == "python":
        if python_harness := _python_harness_entry(repo_root):
            return [str(python_harness), *command[2:]]
    if protocol_bin := _workspace_protocol_bin(repo_root):
        rewritten = [str(protocol_bin), *command[1:]]
        return _append_default_hook_activation(repo_root, command, rewritten)
    rewritten = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(repo_root / "crates" / "agent-semantic-protocol" / "Cargo.toml"),
        "--",
        *command[1:],
    ]
    return _append_default_hook_activation(repo_root, command, rewritten)


def _append_default_hook_activation(
    repo_root: Path, command: list[str], rewritten: list[str]
) -> list[str]:
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


def _workspace_protocol_bin(repo_root: Path) -> Path | None:
    for relative in ("target/debug/asp", ".bin/asp"):
        candidate = repo_root / relative
        if candidate.exists():
            return candidate.resolve()
    return None


def _typescript_harness_dist_entry(repo_root: Path) -> Path | None:
    dist_cli = (
        repo_root
        / "languages"
        / "typescript-lang-project-harness"
        / "dist"
        / "src"
        / "cli"
    )
    for filename in ("main.bundle.js", "main.js"):
        candidate = dist_cli / filename
        if candidate.exists():
            return candidate.resolve()
    return None


def _python_harness_entry(repo_root: Path) -> Path | None:
    candidate = repo_root / ".bin" / "py-harness"
    if candidate.exists():
        return candidate.resolve()
    return None


def _is_hook_command(command: list[str]) -> bool:
    return len(command) > 1 and command[1] == "hook"


def run_step_process(
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
    except subprocess.TimeoutExpired as error:
        return _process_setup_error_result(
            scenario_id=scenario_id,
            step_id=step_id,
            command=command,
            started=started,
            error=f"timeout after {timeout_seconds:g}s",
            stdout=_timeout_text(error.stdout),
            stderr=_timeout_text(error.stderr),
        )
    return process, round((time.perf_counter() - started) * 1000)


def _process_setup_error_result(
    *,
    scenario_id: str,
    step_id: str,
    command: list[str],
    started: float,
    error: str,
    stdout: str = "",
    stderr: str = "",
) -> StepResult:
    observations = summarize_agent_stdout(stdout)
    return StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=command,
        status="fail",
        exit_code=None,
        elapsed_ms=round((time.perf_counter() - started) * 1000),
        stdout_lines=count_lines(stdout),
        stderr_lines=count_lines(stderr),
        stdout_bytes=len(stdout.encode("utf-8")),
        stderr_bytes=len(stderr.encode("utf-8")),
        errors=[error],
        observations=observations,
    )


def _timeout_text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def completed_step_result(
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
