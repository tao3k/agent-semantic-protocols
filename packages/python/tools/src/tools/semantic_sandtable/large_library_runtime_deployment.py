"""Deploy the current workspace providers before runtime measurement."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import time
from typing import Any


def release_binary_is_valid(binary: Path) -> bool:
    if not binary.is_file():
        return False
    completed = subprocess.run(
        [str(binary), "--version", "--require-release"],
        text=True,
        capture_output=True,
        check=False,
        timeout=10,
        env=_automation_environment(),
    )
    return completed.returncode == 0


def install_workspace_providers(
    binary: Path,
    repo_root: Path,
    languages: tuple[str, ...],
) -> list[dict[str, Any]]:
    return [
        install_workspace_provider(binary, repo_root, language)
        for language in languages
    ]


def install_workspace_provider(
    binary: Path,
    repo_root: Path,
    language: str,
) -> dict[str, Any]:
    command = [
        str(binary),
        "install",
        "language",
        language,
        "--from-workspace",
        "--project",
        str(repo_root),
    ]
    public_command = [
        "asp",
        "install",
        "language",
        language,
        "--from-workspace",
        "--project",
        str(repo_root),
    ]
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            cwd=repo_root,
            text=True,
            capture_output=True,
            check=False,
            timeout=120,
            env=_automation_environment(),
        )
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout or ""
        stderr = error.stderr or ""
        return {
            "language": language,
            "command": public_command,
            "status": "fail",
            "elapsedMs": int((time.perf_counter() - started) * 1_000),
            "stdoutBytes": len(stdout.encode()),
            "stderrBytes": len(stderr.encode()),
            "errors": ["workspace-install-timeout"],
        }
    errors = [] if completed.returncode == 0 else [f"exit-code-{completed.returncode}"]
    return {
        "language": language,
        "command": public_command,
        "status": "pass" if not errors else "fail",
        "elapsedMs": int((time.perf_counter() - started) * 1_000),
        "stdoutBytes": len(completed.stdout.encode()),
        "stderrBytes": len(completed.stderr.encode()),
        "errors": errors,
    }


def _automation_environment() -> dict[str, str]:
    return {**os.environ, "ASP_NO_AGENT_PLATFORM": "1"}
