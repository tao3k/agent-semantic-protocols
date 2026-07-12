"""Execute and validate benchmark steps from live provider descriptors."""

from __future__ import annotations

import json
import re
import time
from pathlib import Path
from typing import Any

from .large_library_runtime_invocation import benchmark_command_from_descriptor
from .large_library_runtime_process import facade_environment, run_public_command
from .large_library_runtime_types import CommandResult, Corpus, Invocation


_ERROR_RECEIPT = re.compile(r"(?im)(?:^|\n)\[[^\n]+\]\s+error=|\berror:\s")


def warmup(
    binary: Path,
    corpus: Corpus,
    workspace: Path,
    descriptors: list[dict[str, Any]],
) -> dict[str, Any]:
    descriptor = next(
        (item for item in descriptors if item.get("method") == "search/prime"),
        descriptors[0],
    )
    step = benchmark_step(binary, corpus, workspace, descriptor)
    return {
        "scenarioId": corpus.scenario_id,
        "language": corpus.language,
        "method": step["method"],
        "command": step["command"],
        "status": step["status"],
        "elapsedMs": step["elapsedMs"],
    }


def benchmark_step(
    binary: Path,
    corpus: Corpus,
    workspace: Path,
    descriptor: dict[str, Any],
) -> dict[str, Any]:
    method = str(descriptor.get("method", "search/unknown"))
    try:
        invocation = benchmark_command_from_descriptor(
            descriptor,
            corpus.language,
            {"workspace": str(workspace), **corpus.inputs},
        )
    except ValueError as error:
        return template_failure(corpus, method, str(error))
    return benchmark_invocation_step(binary, corpus, invocation, method)


def benchmark_fd_step(binary: Path, corpus: Corpus, workspace: Path) -> dict[str, Any]:
    """Exercise the public path/module stage once for every real corpus."""
    return benchmark_invocation_step(
        binary,
        corpus,
        Invocation(
            command=["asp", "fd", "-query", corpus.inputs["query"], "--workspace", str(workspace)],
            stdin=None,
            expects_json=False,
            max_elapsed_ms=1_000,
        ),
        "search/fd-path",
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


def template_failure(corpus: Corpus, method: str, error: str) -> dict[str, Any]:
    return {
        "scenarioId": corpus.scenario_id,
        "language": corpus.language,
        "method": method,
        "stepId": method.removeprefix("search/"),
        "command": ["asp", corpus.language, "search"],
        "status": "fail",
        "executed": False,
        "elapsedMs": 0,
        "maxElapsedMs": 0,
        "stdoutBytes": 0,
        "stderrBytes": 0,
        "processTreeTerminated": False,
        "warnings": [],
        "errors": [error],
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
