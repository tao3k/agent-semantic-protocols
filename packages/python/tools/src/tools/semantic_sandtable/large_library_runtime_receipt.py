"""Build the strict receipt for the large-library runtime benchmark."""

from __future__ import annotations

from pathlib import Path
from typing import Any


def empty_coverage() -> dict[str, Any]:
    return {
        "registeredSearchMethodCount": 0,
        "targetSearchCommandCount": 0,
        "runtimeSearchCommandCount": 0,
        "registeredMethods": [],
        "runtimeMethods": [],
        "missingMethods": [],
        "missingCorpusMethods": [],
    }


def coverage(
    registered_methods: set[str], target_command_count: int, steps: list[dict[str, Any]]
) -> dict[str, Any]:
    executed = [
        step
        for step in steps
        if step.get("executed") is True and str(step.get("method", "")).startswith("search/")
    ]
    runtime_methods = {str(step["method"]) for step in executed}
    missing = [
        {
            "scenarioId": str(step["scenarioId"]),
            "language": str(step["language"]),
            "method": str(step["method"]),
        }
        for step in steps
        if str(step.get("method", "")).startswith("search/")
        and step.get("executed") is not True
    ]
    return {
        "registeredSearchMethodCount": len(registered_methods),
        "targetSearchCommandCount": target_command_count,
        "runtimeSearchCommandCount": len(executed),
        "registeredMethods": sorted(registered_methods),
        "runtimeMethods": sorted(runtime_methods),
        "missingMethods": sorted(registered_methods - runtime_methods),
        "missingCorpusMethods": sorted(
            missing,
            key=lambda entry: (entry["language"], entry["scenarioId"], entry["method"]),
        ),
    }


def runtime_receipt(
    *,
    binary: Path,
    release_verified: bool,
    workspace_deployments: list[dict[str, Any]],
    corpora: list[dict[str, str]],
    missing: list[dict[str, str]],
    command_coverage: dict[str, Any],
    warmups: list[dict[str, Any]],
    steps: list[dict[str, Any]],
) -> dict[str, Any]:
    failure_count = (
        sum(1 for step in steps if step["status"] == "fail")
        + len(missing)
        + int(not release_verified)
        + len(command_coverage["missingMethods"])
        + len(command_coverage["missingCorpusMethods"])
        + sum(1 for deployment in workspace_deployments if deployment["status"] == "fail")
    )
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-large-library-runtime-benchmark",
        "schemaVersion": "1",
        "packetKind": "large-library-runtime-benchmark",
        "status": "fail" if failure_count else "pass",
        "binary": {"path": str(binary), "releaseVerified": release_verified},
        "workspaceDeployments": workspace_deployments,
        "corpora": corpora,
        "missingCorpora": missing,
        "commandCoverage": command_coverage,
        "summary": {
            "scenarioCount": len(corpora),
            "stepCount": len(steps),
            "failureCount": failure_count,
            "warningCount": 0,
            "maxElapsedMs": max((int(step["elapsedMs"]) for step in steps), default=0),
        },
        "warmups": warmups,
        "steps": steps,
    }
