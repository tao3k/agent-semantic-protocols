"""Cross-language workspace/search contract tool tests."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Any, NoReturn


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/src"))

from tools.language_workspace_search_contract_gate import run_contract  # noqa: E402
from tools.language_workspace_search_contract_cases import CONTRACT_CASES  # noqa: E402
from tools.language_workspace_search_contract_runner import _run_asp  # noqa: E402
from tools.language_workspace_search_contract_types import AspResult  # noqa: E402


def test_language_workspace_search_contract_runs_expected_matrix(tmp_path: Path) -> None:
    seen: list[tuple[str, ...]] = []

    def fake_run_asp(args, _repo_root, _asp_bin) -> AspResult:
        command = tuple(args)
        seen.append(command)
        if command[-1:] == (".",) and command[1:3] == ("search", "ingest"):
            return AspResult(
                args=command,
                returncode=2,
                stdout="",
                stderr="expected at most one PROJECT_ROOT argument\n",
            )
        if command[1:3] == ("search", "workspace"):
            return AspResult(
                args=command,
                returncode=0,
                stdout=_workspace_stdout(command[0]),
                stderr="",
            )
        if command[1:3] == ("search", "ingest"):
            if command[0] == "julia":
                return AspResult(
                    args=command,
                    returncode=0,
                    stdout="[search-ingest] pipes=owner,tests\n",
                    stderr="",
                )
            return AspResult(
                args=command,
                returncode=0,
                stdout="[search-ingest]\nfrontier=O.owner,T.tests\n",
                stderr="",
            )
        if command[1:3] == ("agent", "doctor"):
            pipes = '["owner","tests"]' if command[0] == "julia" else '["items","tests"]'
            return AspResult(
                args=command,
                returncode=0,
                stdout=f'{{"method":"search/workspace","acceptedPipes":{pipes}}}',
                stderr="",
            )
        raise AssertionError(f"unexpected command: {command}")

    run_contract(repo_root=tmp_path, asp_bin="/bin/asp", run_asp=fake_run_asp)

    assert (
        "rust",
        "search",
        "workspace",
        "--view",
        "seeds",
        "languages/asp-rust",
    ) in seen
    assert (
        "typescript",
        "search",
        "ingest",
        "items",
        "tests",
        "--view",
        "seeds",
        "languages/typescript-lang-project-harness",
    ) in seen
    assert (
        "python",
        "agent",
        "doctor",
        "--json",
        "languages/asp-python",
    ) in seen
    assert (
        "julia",
        "search",
        "ingest",
        "owner",
        "tests",
        "--view",
        "seeds",
        "languages/AspJulia.jl",
        ".",
    ) in seen
    python_case = next(case for case in CONTRACT_CASES if case.language == "python")
    assert python_case.workspace_router_next_prime is True


def test_language_workspace_search_contract_runner_reports_timeout(
    monkeypatch,
    tmp_path: Path,
) -> None:
    def fake_run(*_args: Any, **kwargs: Any) -> NoReturn:
        raise subprocess.TimeoutExpired(
            cmd="/bin/asp",
            timeout=kwargs["timeout"],
            output="partial stdout\n",
            stderr=b"partial stderr\n",
        )

    monkeypatch.setattr(subprocess, "run", fake_run)
    monkeypatch.setenv("SEMANTIC_AGENT_PROTOCOL_BIN", "/bin/asp")

    result = _run_asp(["julia", "search", "workspace"], tmp_path, None)

    assert result.returncode == 124
    assert result.stdout == "partial stdout\n"
    assert "timed out after 60s" in result.stderr
    assert "partial stderr" in result.stderr


def _workspace_stdout(language: str) -> str:
    if language == "rust":
        return (
            "[search-workspace]\n"
            "legend: ID=kind:role(value)!next\n"
            "aliases: graph:{G=search,P=package}\n"
            "P=package:pkg(.)\n"
            "G>{P:selects}\n"
            "frontier=P.owner\n"
        )
    if language == "julia":
        return (
            "[search-workspace]\n"
            "legend: ID=kind:role(value)!next\n"
            "aliases: graph:{G=search,O=owner}\n"
            "O=owner:path(src/Example.jl)!owner\n"
            "G>{O:selects}\n"
            "frontier=O.owner\n"
        )
    if language == "python":
        return (
            "[search-workspace]\n"
            "legend: ID=kind:role(value)!next\n"
            "aliases: graph:{G=search}\n"
            "G>{}\n"
            "rank= frontier=\n"
            "|next prime:.,prime:src/python_lang_parser\n"
        )
    return (
        "[search-workspace]\n"
        "legend: ID=kind:role(value)!next\n"
        "aliases: graph:{G=search,O=owner}\n"
        "O=owner:path(.)!owner\n"
        "G>{O:selects}\n"
        "frontier=O.owner\n"
    )
