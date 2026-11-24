# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Resolve sandtable live OpenAI-compatible agent SDK steps."""

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
from .utils import require_str

_OPENAI_COMPATIBLE_CLIENTS = {"deepseek", "openai-compatible"}


def resolve_agent_sdk_step(
    step: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    captures: dict[str, str],
    repo_root: Path | None,
) -> tuple[list[str], dict[str, str]] | StepResult:
    _ = repo_root
    spec = step.get("agentSdk")
    if not isinstance(spec, dict):
        return empty_step_error(scenario_id, step_id, "step.agentSdk must be an object")
    client = require_str(spec, "client", "")
    if client not in _OPENAI_COMPATIBLE_CLIENTS:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.agentSdk.client must be 'deepseek' or 'openai-compatible'",
        )
    contract_error = _validate_agent_answer_runtime_contract(
        step,
        spec,
        scenario_id,
        step_id,
    )
    if contract_error is not None:
        return contract_error

    resolved = _resolve_openai_compatible_sdk_command(
        client,
        spec,
        scenario_id,
        step_id,
        captures,
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


def _resolve_openai_compatible_sdk_command(
    client: str,
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    command = _base_openai_compatible_command(
        client,
        spec,
        scenario_id,
        step_id,
        captures,
    )
    if isinstance(command, StepResult):
        return command
    for option, field in (
        ("--model", "model"),
        ("--base-url", "baseUrl"),
        ("--api-key-env", "apiKeyEnv"),
    ):
        value = optional_agent_string(
            spec,
            field,
            scenario_id,
            step_id,
            captures,
            "agentSdk",
        )
        if isinstance(value, StepResult):
            return value
        if value:
            command.extend([option, value])
    if bool(spec.get("requireLiveUsage", False)):
        command.append("--require-live-usage")
    return command


def _base_openai_compatible_command(
    client: str,
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
) -> list[str] | StepResult:
    prompt = required_agent_string(
        spec,
        "prompt",
        scenario_id,
        step_id,
        captures,
        "agentSdk",
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
        "tools.semantic_sandtable.openai_compatible_runner",
        "--provider",
        client,
        "--prompt",
        prompt,
        "--output-format",
        output_format,
    ]
