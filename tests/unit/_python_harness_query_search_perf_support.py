"""Support helpers for ASP Python search performance tests."""

from __future__ import annotations

import json
import os
import subprocess
import time
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PY_HARNESS_SRC = REPO_ROOT / "languages/python-lang-project-harness/src"
SEARCH_TERM = "compute_value"
DEPENDENCY = "requests"
WARM_ASP_BUDGET_MS = 1000.0
SEARCH_PREFLIGHT_BUDGET_MS = 50.0
SINGLE_FILE_RG_BUDGET_MS = 100.0


def canonical_asp_binary() -> Path:
    return Path(os.environ.get("ASP_BIN", Path.home() / ".local" / "bin" / "asp"))


@dataclass(frozen=True, slots=True)
class SearchCommand:
    args: list[str]
    stdin: str | None = None


def write_python_fixture(tmp_path: Path) -> Path:
    project = tmp_path / "project"
    source_dir = project / "src"
    source_dir.mkdir(parents=True)
    (project / "pyproject.toml").write_text(
        "[project]\n"
        'name = "perf-fixture"\n'
        'version = "0.1.0"\n'
        'dependencies = ["requests"]\n',
        encoding="utf-8",
    )
    (source_dir / "example.py").write_text(
        "import requests\n\n"
        "def compute_value(item: int) -> int:\n"
        '    response = requests.get("https://example.com")\n'
        "    return item + len(response.text)\n\n"
        "def unrelated() -> int:\n"
        "    return 0\n",
        encoding="utf-8",
    )
    return project


def lexical_command(project: Path) -> list[str]:
    return [
        "search",
        "lexical",
        "--query",
        SEARCH_TERM,
        "--query",
        DEPENDENCY,
        "--workspace",
        str(project),
        "--view",
        "seeds",
    ]


def python_semantic_language_descriptors(project: Path) -> list[dict[str, object]]:
    output = run_asp_python(["agent", "doctor", "--json"], project)
    registry = json.loads(output)
    [language] = [
        entry
        for entry in registry["languages"]
        if entry["languageId"] == "python" and entry["providerId"] == "py-harness"
    ]
    return list(language["methodDescriptors"])


def search_commands_for_descriptor(
    descriptor: dict[str, object],
    project: Path,
) -> Iterator[SearchCommand]:
    workspace = ["--workspace", str(project)]
    owner = str(project / "src" / "example.py")
    view = str(descriptor["view"])

    if view in {"workspace", "prime", "env", "runtime-source", "lang", "std", "capability"}:
        yield SearchCommand(["search", view, *workspace])
    elif view == "owner":
        yield SearchCommand(
            ["search", "owner", owner, "items", "--query", SEARCH_TERM, *workspace]
        )
    elif view == "dependency":
        yield SearchCommand(["search", "dependency", DEPENDENCY, *workspace])
    elif view == "deps":
        yield SearchCommand(["search", "deps", f"{DEPENDENCY}::get", *workspace])
    elif view == "public-external-types":
        yield SearchCommand(["search", view, DEPENDENCY, *workspace])
    elif view in {"api", "symbol"}:
        yield SearchCommand(["search", view, SEARCH_TERM, *workspace])
    elif view == "callsite":
        yield SearchCommand(["search", "callsite", "get", *workspace])
    elif view == "import":
        yield SearchCommand(["search", "import", DEPENDENCY, *workspace])
    elif view == "tests":
        yield SearchCommand(["search", "tests", owner, *workspace])
    elif view == "lexical":
        yield SearchCommand(lexical_command(project))
    elif view == "policy":
        yield SearchCommand(["search", "policy", SEARCH_TERM, *workspace])
    elif view == "reasoning":
        yield from _reasoning_commands(owner, workspace)
    elif view in {"extension", "compare"}:
        yield SearchCommand(["search", view, "typing", *workspace])
    elif view == "pattern":
        yield SearchCommand(["search", "pattern", "context-manager", *workspace])
    elif view == "ingest":
        yield SearchCommand(
            ["search", "ingest", *workspace],
            stdin=f"src/example.py:4:def {SEARCH_TERM}(item: int) -> int:\n",
        )
    elif view == "semantic-facts":
        yield SearchCommand(["search", "semantic-facts", SEARCH_TERM, *workspace, "--json"])
    else:
        raise AssertionError(f"uncovered Python search view: {view}")


def timed_asp_python(
    args: list[str],
    cwd: Path,
    *,
    stdin: str | None = None,
) -> tuple[float, str]:
    started = time.perf_counter()
    output = run_asp_python(args, cwd, stdin=stdin)
    return (time.perf_counter() - started) * 1000.0, output


def require_release_asp() -> None:
    asp_binary = canonical_asp_binary()
    assert asp_binary.is_file(), asp_binary
    result = subprocess.run(
        [str(asp_binary), "--version", "--require-release"],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr


def timed_asp(args: list[str]) -> tuple[float, str]:
    started = time.perf_counter()
    result = subprocess.run(
        [str(canonical_asp_binary()), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
        timeout=1.0,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    assert result.returncode == 0, result.stderr
    return elapsed_ms, result.stdout + result.stderr


def run_asp_python(args: list[str], cwd: Path, *, stdin: str | None = None) -> str:
    result = subprocess.run(
        ["asp", "python", *args],
        cwd=cwd,
        env=_asp_env(args),
        input=stdin,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def _reasoning_commands(owner: str, workspace: list[str]) -> Iterator[SearchCommand]:
    yield SearchCommand(["search", "reasoning", "owner-tests", "--owner", owner, *workspace])
    yield SearchCommand(
        [
            "search",
            "reasoning",
            "owner-query",
            "--owner",
            owner,
            "--query",
            SEARCH_TERM,
            *workspace,
        ]
    )
    yield SearchCommand(
        [
            "search",
            "reasoning",
            "query-deps",
            "--query",
            "get",
            "--dependency",
            DEPENDENCY,
            *workspace,
        ]
    )


def _asp_env(args: list[str]) -> dict[str, str]:
    env = os.environ.copy()
    pythonpath = env.get("PYTHONPATH")
    env["PYTHONPATH"] = (
        str(PY_HARNESS_SRC)
        if not pythonpath
        else f"{PY_HARNESS_SRC}{os.pathsep}{pythonpath}"
    )
    if "--json" in args:
        env["ASP_NO_AGENT_PLATFORM"] = "1"
    return env
