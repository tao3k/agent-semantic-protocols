# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Execute and validate benchmark steps from live provider descriptors."""

from __future__ import annotations

import json
import re
import time
from pathlib import Path
from typing import Any

from .large_library_runtime_process import facade_environment, run_public_command
from .large_library_runtime_types import CommandResult, Corpus, Invocation


_ERROR_RECEIPT = re.compile(r"(?im)(?:^|\n)\[[^\n]+\]\s+error=|\berror:\s")


def warmup(
    binary: Path,
    corpus: Corpus,
    workspace: Path,
) -> dict[str, Any]:
    step = benchmark_playbook_step(binary, corpus, workspace)
    return {
        "scenarioId": corpus.scenario_id,
        "language": corpus.language,
        "method": step["method"],
        "command": step["command"],
        "status": step["status"],
        "elapsedMs": step["elapsedMs"],
    }


def benchmark_playbook_step(
    binary: Path, corpus: Corpus, workspace: Path
) -> dict[str, Any]:
    """Exercise the sole public Search Playbook once for every real corpus."""
    query = corpus.inputs["query"]
    return benchmark_invocation_step(
        binary,
        corpus,
        Invocation(
            command=[
                "asp",
                "search",
                "playbook",
                "--language",
                corpus.language,
                "--workspace",
                str(workspace),
                "--rg",
                "-n",
                "-e",
                query,
                ".",
                "--tantivy",
                "term",
                query,
            ],
            stdin=None,
            expects_json=False,
            max_elapsed_ms=2_500,
        ),
        "search/playbook",
    )


def benchmark_invocation_step(
    binary: Path,
    corpus: Corpus,
    invocation: Invocation,
    method: str,
) -> dict[str, Any]:
    started = time.perf_counter()
    completed = run_public_command(
        [str(binary), *invocation.command[1:]],
        stdin=invocation.stdin,
        timeout_seconds=max(30, (invocation.max_elapsed_ms + 9_999) // 1_000),
        env=facade_environment(invocation.max_elapsed_ms),
    )
    elapsed_ms = int((time.perf_counter() - started) * 1_000)
    errors = ["command-timeout"] if completed.timed_out else invocation_errors(
        completed,
        invocation,
        elapsed_ms,
    )
    return {
        **step_base(
            corpus,
            method,
            invocation,
            elapsed_ms,
            process_tree_terminated=completed.process_tree_terminated,
        ),
        "status": "fail" if errors else "pass",
        "executed": True,
        "stdoutBytes": len(completed.stdout.encode()),
        "stderrBytes": len(completed.stderr.encode()),
        "warnings": [],
        "errors": errors,
    }


def step_base(
    corpus: Corpus,
    method: str,
    invocation: Invocation,
    elapsed_ms: int,
    *,
    process_tree_terminated: bool,
) -> dict[str, Any]:
    return {
        "scenarioId": corpus.scenario_id,
        "language": corpus.language,
        "method": method,
        "stepId": method.removeprefix("search/"),
        "command": invocation.command,
        "elapsedMs": elapsed_ms,
        "maxElapsedMs": invocation.max_elapsed_ms,
        "processTreeTerminated": process_tree_terminated,
    }


def invocation_errors(
    completed: CommandResult,
    invocation: Invocation,
    elapsed_ms: int,
) -> list[str]:
    errors: list[str] = []
    if completed.returncode != 0:
        errors.append(f"exit-code-{completed.returncode}")
    if not completed.stdout.strip():
        errors.append("empty-payload")
    if _ERROR_RECEIPT.search(completed.stdout):
        errors.append("protocol-error-output")
    if invocation.expects_json:
        try:
            json.loads(completed.stdout)
        except json.JSONDecodeError:
            errors.append("invalid-json-payload")
    if elapsed_ms > invocation.max_elapsed_ms:
        errors.append("budget-exceeded")
    return errors
