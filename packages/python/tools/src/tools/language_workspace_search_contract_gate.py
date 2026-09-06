"""Scenario runner for the single public Search Playbook contract gate."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from .language_workspace_search_contract_assertions import assert_failure_contains
from .language_workspace_search_contract_cases import (
    CONTRACT_CASES,
    SearchContractCase,
)
from .language_workspace_search_contract_runner import _run_asp
from .language_workspace_search_contract_types import ContractFailure, RunAsp
from .paths import repo_root as default_repo_root


@dataclass(frozen=True)
class ContractContext:
    root: Path
    asp_bin: str | None
    runner: RunAsp

    def run(self, args: list[str]) -> object:
        return self.runner(args, self.root, self.asp_bin)


def run_contract(
    *,
    repo_root: Path | None = None,
    asp_bin: str | None = None,
    run_asp: RunAsp | None = None,
) -> None:
    context = _contract_context(repo_root, asp_bin, run_asp)
    _validate_search_playbook_contract(context)
    for case in CONTRACT_CASES:
        assert_failure_contains(
            context.run(_workspace_args(case)),
            f"{case.language} removed workspace search",
            "language-first Search was removed",
        )
        assert_failure_contains(
            context.run(_ingest_args(case)),
            f"{case.language} removed ingest search",
            "language-first Search was removed",
        )


def _contract_context(
    repo_root: Path | None,
    asp_bin: str | None,
    run_asp: RunAsp | None,
) -> ContractContext:
    return ContractContext(
        root=(repo_root or default_repo_root()).resolve(),
        asp_bin=asp_bin,
        runner=_run_asp if run_asp is None else run_asp,
    )


def _validate_search_playbook_contract(context: ContractContext) -> None:
    registration = json.loads(
        (context.root / "languages/asp-rust/provider/asp-provider-registration.json")
        .read_text()
    )
    contract = registration.get("searchPlaybookContract")
    if not isinstance(contract, dict) or set(contract) != {
        "contractId",
        "contractVersion",
        "languageId",
        "providerId",
        "syntaxContractId",
        "syntaxContractDigest",
        "projection",
    }:
        raise ContractFailure("rust Search Playbook contract is absent or not closed")
    projection = contract.get("projection")
    if not isinstance(projection, dict) or set(projection) != {"example", "grammar"}:
        raise ContractFailure("Search Playbook projection must be exactly Example + Grammar")
    example = projection["example"]
    grammar = projection["grammar"]
    for value in [example, grammar]:
        if "--intent" in value or "--graph pgql" in value or "next=" in value.lower():
            raise ContractFailure("Search Playbook contract exposes a removed reasoning control")
    for needle in [
        "asp search playbook",
        "--fd",
        "--rg",
        "--tantivy",
        "--syntax rust",
        "--graph gql",
    ]:
        if needle not in example or needle not in grammar:
            raise ContractFailure(f"Search Playbook contract omits {needle!r}")


def _workspace_args(case: SearchContractCase) -> list[str]:
    return [
        case.language,
        "search",
        "workspace",
        "--view",
        "seeds",
        case.project_root,
    ]


def _ingest_args(case: SearchContractCase) -> list[str]:
    return [
        case.language,
        "search",
        "ingest",
        *case.ingest_pipes,
        "--view",
        "seeds",
        case.project_root,
    ]
