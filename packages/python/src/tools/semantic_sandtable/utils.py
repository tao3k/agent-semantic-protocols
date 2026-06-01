"""Small value, path, and capture helpers for sandtable execution."""

from __future__ import annotations

import glob
import os
import re
from pathlib import Path
from typing import Any

from .constants import TOKEN_PATTERN


def resolve_workdir(repo_root: Path, spec: Any) -> Path | None:
    if spec is None:
        return repo_root
    if isinstance(spec, str):
        return resolve_path(repo_root, spec)
    if not isinstance(spec, dict):
        return None

    env_name = spec.get("env")
    if isinstance(env_name, str):
        env_value = os.environ.get(env_name)
        if env_value:
            env_path = resolve_path(repo_root, env_value)
            if env_path and env_path.exists():
                return env_path

    relative = spec.get("relative")
    if isinstance(relative, str):
        relative_path = resolve_path(repo_root, relative)
        if relative_path and relative_path.exists():
            return relative_path

    for pattern in string_list(spec.get("candidates", [])):
        matches = resolve_glob(repo_root, pattern)
        if matches:
            return matches[0]
    return None


def resolve_path(repo_root: Path, value: str) -> Path | None:
    expanded = os.path.expandvars(os.path.expanduser(value))
    path = Path(expanded)
    if not path.is_absolute():
        path = repo_root / path
    return path.resolve()


def resolve_glob(repo_root: Path, pattern: str) -> list[Path]:
    expanded = os.path.expandvars(os.path.expanduser(pattern))
    if not Path(expanded).is_absolute():
        expanded = str(repo_root / expanded)
    matches = [Path(match).resolve() for match in glob.glob(expanded)]
    existing = [match for match in matches if match.exists()]
    return sorted(
        existing,
        key=lambda path: (path.stat().st_mtime, str(path)),
        reverse=True,
    )


def expand_string_list(value: Any, captures: dict[str, str]) -> tuple[list[str], list[str]]:
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


def build_env(value: Any) -> dict[str, str]:
    env = os.environ.copy()
    if not isinstance(value, dict):
        return env
    for key, item in value.items():
        if isinstance(key, str):
            env[key] = os.path.expandvars(str(item))
    return env


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


def count_lines(text: str) -> int:
    if not text:
        return 0
    return len(text.splitlines())
