# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Common sandtable step error construction."""

from __future__ import annotations

from .models import StepResult


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
