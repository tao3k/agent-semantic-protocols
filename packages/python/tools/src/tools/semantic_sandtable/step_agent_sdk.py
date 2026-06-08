"""Resolve sandtable Claude SDK steps."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

from .models import StepResult
from .step_agent_common import (
    optional_agent_string,
    required_agent_string,
    resolve_agent_env,
)
from .step_errors import empty_step_error
from .utils import require_str, string_list


def resolve_agent_sdk_step(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
    repo_root: Path | None,
) -> tuple[list[str], dict[str, str]] | StepResult:
    spec = step.get("agentSdk")
    if not isinstance(spec, dict):
        return empty_step_error(scenario_id, step_id, "step.agentSdk must be an object")
    if require_str(spec, "client", "") != "claude":
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentSdk.client must be 'claude'",
        )
    contract_error = _validate_agent_answer_runtime_contract(
        step,
        spec,
        scenario_id,
        step_id,
    )
    if contract_error is not None:
        return contract_error

    resolved = _resolve_claude_sdk_command(
        spec,
        scenario_id,
        step_id,
        captures,
        repo_root,
    )
    if isinstance(resolved, StepResult):
        return resolved
    step_env = resolve_agent_env(spec, scenario_id, step_id, env, "agentSdk")
    if isinstance(step_env, StepResult):
        return step_env
    return resolved, step_env


def _validate_agent_answer_runtime_contract(
    step: dict[str, Any],
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
) -> StepResult | None:
    expect = step.get("expect")
    if not isinstance(expect, dict):
        return None
    agent_answer = expect.get("agentAnswer")
    if not isinstance(agent_answer, dict):
        return None
    if agent_answer.get("required", True) is False:
        return None
    if "maxTurns" not in spec:
        return None
    return empty_step_error(
        scenario_id,
        step_id,
        "step.agentSdk.maxTurns cannot be used when expect.agentAnswer.required is true; use timeoutSeconds instead",
    )


def _resolve_claude_sdk_command(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
    repo_root: Path | None = None,
) -> list[str] | StepResult:
    command = _base_sdk_command(spec, scenario_id, step_id, captures)
    if isinstance(command, StepResult):
        return command

    _append_sdk_boolean_options(command, spec)
    _append_sdk_tool_options(command, spec)
    if bool(spec.get("requireAspBashCommands", False)):
        command.append("--require-asp-bash-commands")

    setup_error = _append_sdk_runtime_options(
        command,
        spec,
        scenario_id,
        step_id,
        captures,
        repo_root,
    )
    if setup_error is not None:
        return setup_error
    return command


def _base_sdk_command(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    prompt = required_agent_string(
        spec, "prompt", scenario_id, step_id, captures, "agentSdk"
    )
    output_format = required_agent_string(
        spec,
        "outputFormat",
        scenario_id,
        step_id,
        captures,
        "agentSdk",
    )
    if isinstance(prompt, StepResult):
        return prompt
    if isinstance(output_format, StepResult):
        return output_format
    if output_format not in {"text", "json", "stream-json", "summary-json"}:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentSdk.outputFormat must be text, json, stream-json, or summary-json",
        )
    return [
        sys.executable,
        "-m",
        "tools.semantic_sandtable.claude_sdk_runner",
        "--prompt",
        prompt,
        "--output-format",
        output_format,
    ]


def _append_sdk_boolean_options(command: list[str], spec: dict[str, Any]) -> None:
    if bool(spec.get("includePartialMessages", False)):
        command.append("--include-partial-messages")
    if bool(spec.get("includeHookEvents", False)):
        command.append("--include-hook-events")
    if bool(spec.get("verbose", False)):
        command.append("--verbose")


def _append_sdk_tool_options(command: list[str], spec: dict[str, Any]) -> None:
    for tool in string_list(spec.get("allowedTools")):
        command.extend(["--allowed-tool", tool])
    for tool in string_list(spec.get("disallowedTools")):
        command.extend(["--disallowed-tool", tool])


def _append_sdk_runtime_options(
    command: list[str],
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
    repo_root: Path | None,
) -> StepResult | None:
    repo_error = _append_sdk_repo_settings(
        command, spec, scenario_id, step_id, repo_root
    )
    if repo_error is not None:
        return repo_error
    turn_error = _append_sdk_max_turns(command, spec, scenario_id, step_id)
    if turn_error is not None:
        return turn_error

    model = optional_agent_string(
        spec, "model", scenario_id, step_id, captures, "agentSdk"
    )
    if isinstance(model, StepResult):
        return model
    if model:
        command.extend(["--model", model])
    return None


def _append_sdk_repo_settings(
    command: list[str],
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    repo_root: Path | None,
) -> StepResult | None:
    if not bool(spec.get("useRepoClaudeSettings", False)):
        return None
    if repo_root is None:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentSdk.useRepoClaudeSettings requires repo root context",
        )
    command.extend(
        [
            "--claude-cwd",
            str(repo_root),
            "--settings",
            str(repo_root / ".claude" / "settings.json"),
            "--add-cwd-dir",
        ]
    )
    return None


def _append_sdk_max_turns(
    command: list[str],
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
) -> StepResult | None:
    max_turns = spec.get("maxTurns")
    if max_turns is None:
        return None
    if not isinstance(max_turns, int) or max_turns <= 0:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentSdk.maxTurns must be a positive integer",
        )
    command.extend(["--max-turns", str(max_turns)])
    return None
