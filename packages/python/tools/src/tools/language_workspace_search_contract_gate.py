# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    _validate_no_provider_local_search_playbook_contract(context)
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


def _validate_no_provider_local_search_playbook_contract(context: ContractContext) -> None:
    registration = json.loads(
        (context.root / "languages/asp-rust/provider/asp-provider-registration.json")
        .read_text()
    )
    if "searchPlaybookContract" in registration:
        raise ContractFailure(
            "provider registration retains removed provider-local Search Playbook authority"
        )


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
