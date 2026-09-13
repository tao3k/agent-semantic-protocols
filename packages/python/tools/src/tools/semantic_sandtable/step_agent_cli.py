# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Resolve sandtable Claude CLI steps."""

from __future__ import annotations

from typing import Any

from .models import StepResult
from .step_agent_common import (
    optional_agent_string,
    required_agent_string,
    resolve_agent_env,
)
from .step_errors import empty_step_error
from .utils import require_str


def resolve_agent_cli_step(
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
    step_env = resolve_agent_env(spec, scenario_id, step_id, env, "agentCli")
    if isinstance(step_env, StepResult):
        return step_env
    return resolved, step_env


def _resolve_claude_cli_command(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    binary = required_agent_string(
        spec, "binary", scenario_id, step_id, captures, "agentCli"
    )
    prompt = required_agent_string(
        spec, "prompt", scenario_id, step_id, captures, "agentCli"
    )
    output_format = required_agent_string(
        spec,
        "outputFormat",
        scenario_id,
        step_id,
        captures,
        "agentCli",
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
    input_error = _append_cli_input_format(command, spec, scenario_id, step_id, captures)
    if input_error is not None:
        return input_error
    _append_cli_boolean_options(command, spec)

    model = optional_agent_string(
        spec, "model", scenario_id, step_id, captures, "agentCli"
    )
    if isinstance(model, StepResult):
        return model
    if model:
        command.extend(["--model", model])
    return command


def _append_cli_input_format(
    command: list[str],
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> StepResult | None:
    input_format = optional_agent_string(
        spec,
        "inputFormat",
        scenario_id,
        step_id,
        captures,
        "agentCli",
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
    return None


def _append_cli_boolean_options(command: list[str], spec: dict[str, Any]) -> None:
    if bool(spec.get("includePartialMessages", False)):
        command.append("--include-partial-messages")
    if bool(spec.get("includeHookEvents", False)):
        command.append("--include-hook-events")
    if bool(spec.get("verbose", False)):
        command.append("--verbose")
