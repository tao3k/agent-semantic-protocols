# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""ASP subprocess runner for the language workspace/search contract gate."""

from __future__ import annotations

import os
import shutil
import subprocess
from collections.abc import Sequence
from pathlib import Path

from .language_workspace_search_contract_types import AspResult, ContractFailure

_DEFAULT_TIMEOUT_SECONDS = 60


def _run_asp(args: Sequence[str], root: Path, asp_bin: str | None) -> AspResult:
    command = [*_asp_command(root, asp_bin), *args]
    timeout_seconds = _timeout_seconds()
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        return AspResult(
            args=tuple(args),
            returncode=124,
            stdout=_coerce_output(getattr(exc, "stdout", None) or getattr(exc, "output", None)),
            stderr=(
                f"asp contract command timed out after {timeout_seconds}s: "
                f"{' '.join(command)}\n"
                f"{_coerce_output(getattr(exc, 'stderr', None))}"
            ),
        )
    return AspResult(
        args=tuple(args),
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def _asp_command(root: Path, asp_bin: str | None) -> list[str]:
    if asp_bin:
        return [asp_bin]
    env_bin = os.environ.get("SEMANTIC_AGENT_PROTOCOL_BIN")
    if env_bin:
        return [env_bin]
    if shutil.which("cargo"):
        return [
            "cargo",
            "run",
            "-q",
            "-p",
            "agent-semantic-client",
            "--bin",
            "asp",
            "--",
        ]
    debug_asp = root / "target/debug/asp"
    if debug_asp.is_file():
        return [str(debug_asp)]
    raise ContractFailure("missing cargo and SEMANTIC_AGENT_PROTOCOL_BIN for asp contract gate")


def _timeout_seconds() -> int:
    value = os.environ.get("ASP_LANGUAGE_WORKSPACE_CONTRACT_TIMEOUT_SECONDS")
    if not value:
        return _DEFAULT_TIMEOUT_SECONDS
    try:
        parsed = int(value)
    except ValueError:
        return _DEFAULT_TIMEOUT_SECONDS
    return max(parsed, 1)


def _coerce_output(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode(errors="replace")
    return value
