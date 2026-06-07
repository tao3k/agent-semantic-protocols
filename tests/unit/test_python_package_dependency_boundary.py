"""Python workspace dependency and passive harness policy contracts."""

from __future__ import annotations

import importlib.metadata as metadata
import os
import subprocess
import sys
import tomllib
from pathlib import Path

from python_lang_project_harness import (
    PythonVerificationTaskKind,
    plan_python_project_verification,
    read_python_project_harness_config,
)


_REPO_ROOT = Path(__file__).resolve().parents[2]
_PYTHON_WORKSPACE = _REPO_ROOT / "packages" / "python"
_PERFORMANCE_OWNER = "packages/python/tools/src/tools/julia_cache_performance.py"


def test_python_workspace_dev_depends_on_harness_library() -> None:
    project = tomllib.loads(
        (_PYTHON_WORKSPACE / "pyproject.toml").read_text(encoding="utf-8")
    )
    dev_dependencies = project.get("dependency-groups", {}).get("dev", [])
    sources = project.get("tool", {}).get("uv", {}).get("sources", {})

    assert any(
        _dependency_name(value) == "python-lang-project-harness"
        for value in dev_dependencies
    )
    assert "python-lang-project-harness" in sources


def test_package_env_installs_harness_library_and_pytest_plugin() -> None:
    distributions = {
        distribution.metadata["Name"].lower()
        for distribution in metadata.distributions()
    }
    plugins = [
        f"{entry_point.name}={entry_point.value}"
        for entry_point in metadata.entry_points(group="pytest11")
        if entry_point.name == "python_lang_project_harness"
        or "python_lang_project_harness" in entry_point.value
    ]

    assert "python-lang-project-harness" in distributions
    assert plugins == [
        "python_lang_project_harness=python_lang_project_harness.pytest_plugin"
    ]


def test_pytest_collects_passive_harness_item() -> None:
    env = os.environ.copy()
    env.pop("PYTEST_DISABLE_PLUGIN_AUTOLOAD", None)
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "pytest",
            "--collect-only",
            "-q",
            Path(__file__).relative_to(_REPO_ROOT).as_posix(),
        ],
        cwd=_REPO_ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )

    assert "python-project-harness" in result.stdout


def test_root_harness_verification_profile_enables_perf_task() -> None:
    config = read_python_project_harness_config(_REPO_ROOT)

    assert config is not None
    hints = config.verification_policy.profile_hints
    assert hints
    assert all(hint.verification_tasks_enabled for hint in hints)
    assert any(
        hint.owner_path == _PERFORMANCE_OWNER
        and {responsibility.value for responsibility in hint.responsibilities}
        == {"public_api", "performance"}
        for hint in hints
    )

    plan = plan_python_project_verification(_REPO_ROOT)
    active_tasks = {(task.owner_path, task.kind) for task in plan.active_tasks}

    assert (
        _PERFORMANCE_OWNER,
        PythonVerificationTaskKind.PERFORMANCE,
    ) in active_tasks
    assert any(kind == PythonVerificationTaskKind.REGRESSION for _, kind in active_tasks)


def _dependency_name(value: object) -> str:
    if not isinstance(value, str):
        return ""
    return value.split("[", 1)[0].split(";", 1)[0].split(" ", 1)[0].lower()
