# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Shared helpers for sandtable agent steps."""

from __future__ import annotations

import os
import re
from typing import Any

from .models import StepResult
from .step_errors import empty_step_error
from .utils import expand_tokens, string_list


_ENV_REFERENCE_PATTERN = re.compile(
    r"\$(?:\{[A-Za-z_][A-Za-z0-9_]*\}|[A-Za-z_][A-Za-z0-9_]*)"
)


def required_agent_string(
    spec: dict[str, Any],
    key: str,
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
    field_name: str,
) -> str | StepResult:
    value = spec.get(key)
    if not isinstance(value, str) or not value:
        return empty_step_error(
            scenario_id,
            step_id,
            f"step.{field_name}.{key} must be a non-empty string",
        )
    try:
        return expand_tokens(value, captures)
    except KeyError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"missing capture {error.args[0]!r}",
        )


def optional_agent_string(
    spec: dict[str, Any],
    key: str,
    scenario_id: str,
    step_id: str,
    captures: dict[str, str],
    field_name: str,
) -> str | StepResult | None:
    value = spec.get(key)
    if value is None:
        return None
    if not isinstance(value, str):
        return empty_step_error(
            scenario_id,
            step_id,
            f"step.{field_name}.{key} must be a string",
        )
    try:
        return expand_tokens(value, captures)
    except KeyError as error:
        return empty_step_error(
            scenario_id,
            step_id,
            f"missing capture {error.args[0]!r}",
        )


def resolve_agent_env(
    spec: dict[str, Any],
    scenario_id: str,
    step_id: str,
    env: dict[str, str],
    field_name: str,
) -> dict[str, str] | StepResult:
    step_env = env.copy()
    overrides = spec.get("env", {})
    if not isinstance(overrides, dict):
        return empty_step_error(
            scenario_id,
            step_id,
            f"step.{field_name}.env must be an object",
        )
    for key, value in overrides.items():
        if not isinstance(key, str) or not isinstance(value, str):
            return empty_step_error(
                scenario_id,
                step_id,
                f"step.{field_name}.env entries must be string to string",
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
            f"step.{field_name}.requiredEnv unresolved: " + ", ".join(missing),
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
