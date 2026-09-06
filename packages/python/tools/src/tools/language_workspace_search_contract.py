# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Single public Search Playbook contract gate."""

from __future__ import annotations

import argparse
import os
import sys
from collections.abc import Sequence
from pathlib import Path

from .console import emit
from .language_workspace_search_contract_gate import run_contract
from .language_workspace_search_contract_types import ContractFailure
from .paths import repo_root as default_repo_root


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    try:
        run_contract(
            repo_root=args.repo_root,
            asp_bin=args.asp_bin,
        )
    except ContractFailure as error:
        emit(error, file=sys.stderr)
        return 1
    emit("language Search Playbook contract is valid")
    return 0


def _parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="python -m tools validate language-workspace-search-contract",
        description="Validate the single public Search Playbook contract.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=default_repo_root(),
        help="Repository root containing language provider checkouts.",
    )
    parser.add_argument(
        "--asp-bin",
        default=os.environ.get("SEMANTIC_AGENT_PROTOCOL_BIN"),
        help=(
            "Path to an asp binary. Defaults to SEMANTIC_AGENT_PROTOCOL_BIN, "
            "cargo run, or target/debug/asp."
        ),
    )
    return parser.parse_args(list(argv))


if __name__ == "__main__":
    raise SystemExit(main())
