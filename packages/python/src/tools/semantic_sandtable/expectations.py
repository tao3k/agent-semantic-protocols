"""Step expectation validation for semantic sandtable runs."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from .budgets import warn_if_over
from .guide_quality import validate_guide_quality
from .json_expectations import validate_stdout_json
from .line_protocol import validate_line_protocol
from .models import StepResult
from .utils import string_list


def validate_step(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    stderr: str,
    repo_root: Path,
) -> None:
    expect = step.get("expect", {})
    if not isinstance(expect, dict):
        result.errors.append("step.expect must be an object")
        return

    _validate_exit_code(expect, result)
    _validate_stdout_expectations(expect, result, stdout)
    _validate_stderr_expectations(expect, result, stderr)
    validate_stdout_json(expect, result, stdout, repo_root)
    validate_guide_quality(expect, result, stdout)
    _validate_line_protocol_expectation(expect, result, stdout)
    _validate_budget_warnings(expect, result)


def _validate_exit_code(expect: dict[str, Any], result: StepResult) -> None:
    expected_exit = int(expect.get("exitCode", 0))
    if result.exit_code != expected_exit:
        result.errors.append(
            f"exitCode={result.exit_code} expected={expected_exit}"
        )


def _validate_stdout_expectations(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    for needle in string_list(expect.get("stdoutContains", [])):
        if needle not in stdout:
            result.errors.append(f"stdout missing {needle!r}")
    for needle in string_list(expect.get("stdoutNotContains", [])):
        if needle in stdout:
            result.errors.append(f"stdout unexpectedly contains {needle!r}")
    for pattern in string_list(expect.get("stdoutMatches", [])):
        if re.search(pattern, stdout, flags=re.MULTILINE) is None:
            result.errors.append(f"stdout regex missed {pattern!r}")
    if bool(expect.get("stdoutEmpty", False)) and stdout:
        result.errors.append("stdout expected empty")


def _validate_stderr_expectations(
    expect: dict[str, Any],
    result: StepResult,
    stderr: str,
) -> None:
    for needle in string_list(expect.get("stderrContains", [])):
        if needle not in stderr:
            result.errors.append(f"stderr missing {needle!r}")


def _validate_line_protocol_expectation(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    if bool(expect.get("lineProtocol", False)):
        validate_line_protocol(result, stdout)


def _validate_budget_warnings(expect: dict[str, Any], result: StepResult) -> None:
    warn_if_over(
        result,
        "stdoutLines",
        result.stdout_lines,
        "maxStdoutLinesWarn",
        expect.get("maxStdoutLinesWarn"),
    )
    warn_if_over(
        result,
        "stderrLines",
        result.stderr_lines,
        "maxStderrLinesWarn",
        expect.get("maxStderrLinesWarn"),
    )
    warn_if_over(
        result,
        "stdoutBytes",
        result.stdout_bytes,
        "maxStdoutBytesWarn",
        expect.get("maxStdoutBytesWarn"),
    )
    warn_if_over(
        result,
        "elapsedMs",
        result.elapsed_ms,
        "maxElapsedMsWarn",
        expect.get("maxElapsedMsWarn"),
    )


def capture_values(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    captures: dict[str, str],
) -> None:
    capture_spec = step.get("capture", {})
    if not isinstance(capture_spec, dict):
        return
    for name, pattern in capture_spec.items():
        if not isinstance(name, str) or not isinstance(pattern, str):
            result.errors.append("capture entries must be string to regex")
            continue
        match = re.search(pattern, stdout, flags=re.MULTILINE)
        if match is None:
            result.errors.append(f"capture {name!r} missed {pattern!r}")
            continue
        captures[name] = match.group(1) if match.groups() else match.group(0)
