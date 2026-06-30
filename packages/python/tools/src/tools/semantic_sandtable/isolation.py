"""Build isolated execution environments for semantic sandtable runs."""

from __future__ import annotations

import time
from pathlib import Path
from typing import Any

from .utils import require_str


def scenario_isolation_env(
    repo_root: Path,
    path: Path,
    scenario: dict[str, Any],
    env: dict[str, str],
) -> tuple[dict[str, str], dict[str, Any] | None]:
    """Return a scenario-scoped environment and public isolation evidence."""
    if not _scenario_isolation_enabled(scenario):
        return env, None

    scenario_id = require_str(scenario, "id", path.stem)
    isolation_root = (
        repo_root
        / ".cache"
        / "agent-semantic-protocol"
        / "sandtable"
        / "runs"
        / f"{_safe_scenario_path_id(scenario_id)}-{time.time_ns()}"
    )
    home = isolation_root / "home"
    cache = isolation_root / "cache"
    tmp = isolation_root / "tmp"
    config = isolation_root / "config"
    data = isolation_root / "data"
    state = isolation_root / "state"
    for directory in (home, cache, tmp, config, data, state):
        directory.mkdir(parents=True, exist_ok=True)

    isolated = env.copy()
    isolated.update(_scenario_isolation_values(isolation_root, home, cache, tmp))
    isolated["PATH"] = _prepend_path_entry(
        home / ".local" / "bin", isolated.get("PATH")
    )

    return isolated, _scenario_isolation_evidence(
        repo_root,
        isolated,
        {
            "root": isolation_root,
            "home": home,
            "cache": cache,
            "tmp": tmp,
        },
    )


def _scenario_isolation_enabled(scenario: dict[str, Any]) -> bool:
    isolation = scenario.get("isolation")
    if isolation is False:
        return False
    if isinstance(isolation, dict) and isolation.get("enabled") is False:
        return False
    return True


def _safe_scenario_path_id(value: str) -> str:
    safe = "".join(
        character if character.isalnum() or character in "._-" else "-"
        for character in value
    ).strip(".-")
    return safe or "scenario"


def _prepend_path_entry(entry: Path, existing: str | None) -> str:
    entry_text = str(entry)
    if not existing:
        return entry_text
    entries = existing.split(":")
    if entry_text in entries:
        return existing
    return entry_text + ":" + existing


def _scenario_isolation_values(
    isolation_root: Path,
    home: Path,
    cache: Path,
    tmp: Path,
) -> dict[str, str]:
    return {
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
        "XDG_CACHE_HOME": str(cache),
        "XDG_CONFIG_HOME": str(isolation_root / "config"),
        "XDG_DATA_HOME": str(isolation_root / "data"),
        "XDG_STATE_HOME": str(isolation_root / "state"),
        "ASP_SANDBOX_ROOT": str(isolation_root),
        "ASP_SANDBOX_HOME": str(home),
        "ASP_SANDBOX_CACHE": str(cache),
        "GOCACHE": str(cache / "go-build"),
        "GOMODCACHE": str(cache / "go-mod"),
        "JULIA_DEPOT_PATH": str(isolation_root / "julia-depot"),
        "MIX_HOME": str(isolation_root / "mix"),
        "HEX_HOME": str(isolation_root / "hex"),
        "NPM_CONFIG_CACHE": str(cache / "npm"),
        "PIP_CACHE_DIR": str(cache / "pip"),
        "POETRY_CACHE_DIR": str(cache / "poetry"),
        "UV_CACHE_DIR": str(cache / "uv"),
    }


def _scenario_isolation_evidence(
    repo_root: Path,
    env: dict[str, str],
    paths: dict[str, Path],
) -> dict[str, Any]:
    return {
        "enabled": True,
        "scope": "scenario",
        "paths": {
            name: _public_path_text(str(value), repo_root, env)
            for name, value in paths.items()
        },
        "env": sorted(
            key
            for key in (
                "HOME",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "JULIA_DEPOT_PATH",
                "NPM_CONFIG_CACHE",
                "UV_CACHE_DIR",
            )
            if key in env
        ),
    }


def _public_path_text(text: str, repo_root: Path, env: dict[str, str]) -> str:
    replacements: list[tuple[str, str]] = []
    for raw_path, marker in (
        (str(repo_root.resolve()), "$ASP_REPO_ROOT"),
        (str(repo_root), "$ASP_REPO_ROOT"),
        (env.get("HOME", ""), "$HOME"),
    ):
        if raw_path:
            replacements.append((raw_path, marker))

    public = text
    for raw_path, marker in sorted(set(replacements), key=lambda item: -len(item[0])):
        public = public.replace(raw_path, marker)
    return public
