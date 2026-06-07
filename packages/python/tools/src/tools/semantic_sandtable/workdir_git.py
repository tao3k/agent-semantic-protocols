"""Git checkout workdir resolution for sandtable scenarios."""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path
from typing import Any

_VALID_CACHE_KEY_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")


def resolve_git_workdir(
    repo_root: Path, spec: dict[str, Any], env: dict[str, str]
) -> Path | None:
    git_spec = _git_workdir_spec(spec)
    if git_spec is None:
        return None
    target = _git_checkout_target(repo_root, git_spec)
    if target is None:
        return None
    return _resolved_git_subdir(target, git_spec, env)


def _git_workdir_spec(spec: dict[str, Any]) -> dict[str, Any] | None:
    git_spec = spec.get("git")
    if not isinstance(git_spec, dict):
        return None
    url = git_spec.get("url")
    cache_key = git_spec.get("cacheKey")
    ref = git_spec.get("ref")
    if not isinstance(url, str) or not url:
        return None
    if not isinstance(cache_key, str) or not _VALID_CACHE_KEY_RE.fullmatch(cache_key):
        return None
    if ref is not None and not isinstance(ref, str):
        return None
    return git_spec


def _git_checkout_target(repo_root: Path, git_spec: dict[str, Any]) -> Path | None:
    url = str(git_spec["url"])
    cache_key = str(git_spec["cacheKey"])
    ref = git_spec.get("ref")
    depth = _optional_int(git_spec.get("depth"))
    cache_root = repo_root / ".cache" / "sandtable-repos"
    target = (cache_root / cache_key).resolve()
    if not _ensure_git_checkout(
        target, url, ref if isinstance(ref, str) else None, depth
    ):
        return None
    return target


def _resolved_git_subdir(
    target: Path,
    git_spec: dict[str, Any],
    env: dict[str, str],
) -> Path | None:
    subdir = git_spec.get("subdir", ".")
    if not isinstance(subdir, str):
        return None
    workdir = _resolve_path(target, subdir, env)
    if workdir is None or not _is_relative_to(workdir, target) or not workdir.exists():
        return None
    return workdir


def _ensure_git_checkout(
    target: Path, url: str, ref: str | None, depth: int | None
) -> bool:
    if (target / ".git").exists():
        return True
    if target.exists():
        return False
    target.parent.mkdir(parents=True, exist_ok=True)
    command = ["git", "clone"]
    if depth is not None and depth > 0:
        command.extend(["--depth", str(depth)])
    if ref:
        command.extend(["--branch", ref])
    command.extend([url, str(target)])
    return (
        subprocess.run(
            command,
            text=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        == 0
    )


def _resolve_path(repo_root: Path, value: str, env: dict[str, str]) -> Path:
    expanded = _expand_env_vars(os.path.expanduser(value), env)
    path = Path(expanded)
    if not path.is_absolute():
        path = repo_root / path
    return path.resolve()


def _expand_env_vars(value: str, env: dict[str, str]) -> str:
    return re.sub(
        r"\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))",
        lambda match: env.get(
            match.group(1) or match.group(2),
            match.group(0),
        ),
        value,
    )


def _optional_int(value: Any) -> int | None:
    if isinstance(value, int):
        return value
    if isinstance(value, str) and value.isdigit():
        return int(value)
    return None


def _is_relative_to(path: Path, base: Path) -> bool:
    try:
        path.relative_to(base)
    except ValueError:
        return False
    return True
