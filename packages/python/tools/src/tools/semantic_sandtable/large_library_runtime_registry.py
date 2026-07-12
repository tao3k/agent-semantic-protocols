"""Read the live provider registry before command-set execution."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .large_library_runtime_process import facade_environment, run_public_command
from .large_library_runtime_types import CommandResult, Corpus


def search_descriptors(
    binary: Path,
    corpus: Corpus,
    workspace: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any] | None]:
    public_command = ["asp", corpus.language, "agent", "doctor", "--json", str(workspace)]
    completed = run_public_command(
        [str(binary), *public_command[1:]],
        timeout_seconds=30,
        env=facade_environment(30_000),
    )
    if completed.timed_out:
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-timeout")
    if completed.returncode != 0:
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-failed")
    try:
        registry = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-invalid-json")
    languages = registry.get("languages")
    if not isinstance(languages, list):
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-missing-languages")
    registrations = [
        entry
        for entry in languages
        if isinstance(entry, dict) and entry.get("languageId") == corpus.language
    ]
    if len(registrations) != 1:
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-language-drift")
    descriptors = [
        descriptor
        for descriptor in registrations[0].get("methodDescriptors", [])
        if isinstance(descriptor, dict)
        and isinstance(descriptor.get("method"), str)
        and descriptor["method"].startswith("search/")
    ]
    if not descriptors:
        return [], registry_failure(corpus, public_command, completed, "agent-doctor-missing-search")
    return sorted(descriptors, key=lambda descriptor: str(descriptor["method"])), None


def registry_failure(
    corpus: Corpus,
    command: list[str],
    completed: CommandResult,
    error: str,
) -> dict[str, Any]:
    return {
        "scenarioId": corpus.scenario_id,
        "language": corpus.language,
        "method": "agent/doctor",
        "stepId": "registry",
        "command": command,
        "status": "fail",
        "executed": True,
        "elapsedMs": 0,
        "maxElapsedMs": 30_000,
        "stdoutBytes": len(completed.stdout.encode()),
        "stderrBytes": len(completed.stderr.encode()),
        "processTreeTerminated": completed.process_tree_terminated,
        "warnings": [],
        "errors": [error],
    }
