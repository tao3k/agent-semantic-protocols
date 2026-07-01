"""Shared support for tree-sitter rollout contract gates."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tools.paths import repo_root


ROOT = repo_root()


class ContractFailure(AssertionError):
    """Raised when a rollout contract assertion fails."""


def asp(env: dict[str, str], asp_bin: str, *args: str) -> str:
    return run([asp_bin, *args], env=env).stdout


def asp_expect_fail(env: dict[str, str], asp_bin: str, *args: str) -> str:
    completed = subprocess.run(
        [asp_bin, *args],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if completed.returncode == 0:
        raise ContractFailure(
            f"expected command to fail: {asp_bin} {' '.join(args)}\n{completed.stdout}"
        )
    return completed.stdout + completed.stderr


def run(
    command: list[str],
    *,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ContractFailure(
            f"command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def contains(value: str, needle: str, label: str) -> None:
    if needle not in value:
        raise ContractFailure(f"{label}: expected output to contain {needle!r}\n{value}")


def not_contains(value: str, needle: str, label: str) -> None:
    if needle in value:
        raise ContractFailure(f"{label}: expected output to omit {needle!r}\n{value}")


def json_string(value: str, key: str, expected: str, label: str) -> None:
    compact = f'"{key}":"{expected}"'
    pretty = f'"{key}": "{expected}"'
    if compact not in value and pretty not in value:
        raise ContractFailure(f"{label}: expected JSON field {key}={expected!r}\n{value}")


def json_false(value: str, key: str, label: str) -> None:
    if f'"{key}":false' not in value and f'"{key}": false' not in value:
        raise ContractFailure(f"{label}: expected JSON field {key}=false\n{value}")


def no_cache_noise(value: str, label: str) -> None:
    for needle in ("artifactId", "sqlite", "cacheRoot", "receipt="):
        not_contains(value, needle, label)


def pure_code(value: str, signature: str, label: str) -> None:
    contains(value, signature, label)
    for needle in (
        "[query-treesitter]",
        "[read-owner]",
        "[read-plan]",
        "|code",
        "text=",
        "frontier=",
    ):
        not_contains(value, needle, label)
    no_cache_noise(value, label)


def search_frontier(value: str, label: str) -> None:
    contains(value, "[search-fzf]", label)
    contains(value, "legend:", label)
    contains(value, "frontier ID.next", label)
    contains(value, "frontier=", label)
    contains(value, "entries=owner-query", label)
    no_cache_noise(value, label)


def root_relative(path: Path) -> str:
    return str(path.relative_to(ROOT))
