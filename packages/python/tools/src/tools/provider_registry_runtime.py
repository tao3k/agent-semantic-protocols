"""Provider registry command runtime helpers."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True, slots=True)
class RegistryResult:
    registry: dict[str, Any] | None = None
    error: str | None = None


def resolve_asp_bin(configured: str | None) -> str:
    candidate = configured or os.environ.get("SEMANTIC_AGENT_PROTOCOL_BIN") or "asp"
    if "/" in candidate:
        return candidate
    resolved = shutil.which(candidate)
    if resolved is None:
        raise SystemExit(f"asp binary not found: {candidate}")
    return resolved


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise SystemExit(f"{path}: expected JSON object")
    return data


def provider_registry(asp_bin: str, provider: str, repo_root: Path) -> RegistryResult:
    argv = [asp_bin, provider, "agent", "doctor", "--json", str(repo_root)]
    try:
        completed = subprocess.run(
            argv,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return RegistryResult(error=f"{provider}: doctor command timed out after 30s")

    if completed.returncode != 0:
        stderr = completed.stderr.strip().splitlines()
        detail = stderr[-1] if stderr else f"exit={completed.returncode}"
        return RegistryResult(error=f"{provider}: doctor command failed: {detail}")

    try:
        registry = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        return RegistryResult(error=f"{provider}: invalid JSON: {error}")

    if not isinstance(registry, dict):
        return RegistryResult(error=f"{provider}: registry JSON must be an object")
    return RegistryResult(registry=registry)
