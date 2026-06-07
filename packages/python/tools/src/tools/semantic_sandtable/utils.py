"""Small value, path, and capture helpers for sandtable execution."""

from __future__ import annotations

import glob
import os
import re
from pathlib import Path
from typing import Any

from .constants import TOKEN_PATTERN
from .workdir_git import resolve_git_workdir


def resolve_workdir(repo_root: Path, spec: Any) -> Path | None:
    return resolve_workdir_with_env(repo_root, spec, os.environ)


def resolve_workdir_with_env(
    repo_root: Path, spec: Any, env: dict[str, str]
) -> Path | None:
    if spec is None:
        return repo_root
    if isinstance(spec, str):
        return resolve_path(repo_root, spec, env)
    if not isinstance(spec, dict):
        return None

    env_name = spec.get("env")
    if isinstance(env_name, str):
        env_value = env.get(env_name)
        if env_value:
            env_path = resolve_path(repo_root, env_value, env)
            if env_path and env_path.exists():
                return env_path

    relative = spec.get("relative")
    if isinstance(relative, str):
        relative_path = resolve_path(repo_root, relative, env)
        if relative_path and relative_path.exists():
            return relative_path

    git_workdir = resolve_git_workdir(repo_root, spec, env)
    if git_workdir is not None:
        return git_workdir
    for pattern in string_list(spec.get("candidates", [])):
        matches = resolve_glob(repo_root, pattern, env)
        if matches:
            return matches[0]
    return None


def resolve_path(
    repo_root: Path, value: str, env: dict[str, str] | None = None
) -> Path | None:
    expanded = _expand_env_vars(os.path.expanduser(value), env or os.environ)
    path = Path(expanded)
    if not path.is_absolute():
        path = repo_root / path
    return path.resolve()


def resolve_glob(
    repo_root: Path, pattern: str, env: dict[str, str] | None = None
) -> list[Path]:
    expanded = _expand_env_vars(os.path.expanduser(pattern), env or os.environ)
    if not Path(expanded).is_absolute():
        expanded = str(repo_root / expanded)
    matches = [Path(match).resolve() for match in glob.glob(expanded)]
    existing = [match for match in matches if match.exists()]
    return sorted(
        existing,
        key=lambda path: (path.stat().st_mtime, str(path)),
        reverse=True,
    )


def _expand_env_vars(value: str, env: dict[str, str]) -> str:
    return re.sub(
        r"\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))",
        lambda match: env.get(
            match.group(1) or match.group(2),
            match.group(0),
        ),
        value,
    )


def expand_string_list(
    value: Any, captures: dict[str, str]
) -> tuple[list[str], list[str]]:
    raw_items = string_list(value)
    errors: list[str] = []
    expanded: list[str] = []
    for item in raw_items:
        try:
            expanded.append(expand_tokens(item, captures))
        except KeyError as error:
            errors.append(f"missing capture {error.args[0]!r}")
    return expanded, errors


def expand_tokens(value: str, captures: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        if name not in captures:
            raise KeyError(name)
        return captures[name]

    return TOKEN_PATTERN.sub(replace, value)


def build_env(value: Any, *, repo_root: Path | None = None) -> dict[str, str]:
    env = os.environ.copy()
    if isinstance(value, dict):
        for key, item in value.items():
            if isinstance(key, str):
                env[key] = os.path.expandvars(str(item))
    _set_workspace_protocol_bin(env, repo_root)
    return env


def _set_workspace_protocol_bin(env: dict[str, str], repo_root: Path | None) -> None:
    if "SEMANTIC_AGENT_PROTOCOL_BIN" in env or repo_root is None:
        return
    protocol_bin = _workspace_protocol_bin(repo_root)
    if protocol_bin is not None:
        env["SEMANTIC_AGENT_PROTOCOL_BIN"] = str(protocol_bin)


def _workspace_protocol_bin(repo_root: Path) -> Path | None:
    for relative in (
        "target/debug/asp",
        "target/debug/semantic-agent-protocol",
        ".bin/asp",
    ):
        candidate = repo_root / relative
        if candidate.exists():
            return candidate.resolve()
    return None


def require_str(mapping: dict[str, Any], key: str, default: str) -> str:
    value = mapping.get(key, default)
    if isinstance(value, str):
        return value
    return default


def string_list(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def dict_value(value: Any) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    return {}


def list_value(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    return []


def optional_int(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def optional_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def count_lines(text: str) -> int:
    if not text:
        return 0
    return len(text.splitlines())
