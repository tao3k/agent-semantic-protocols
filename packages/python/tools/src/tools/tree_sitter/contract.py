"""Shared tree-sitter grammar-profile contract checks."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Iterable

from tools.console import emit
from tools.paths import repo_root

REPO_ROOT = repo_root()

COMMON_CONTRACT_PATHS = (
    "schemas/semantic-tree-sitter-grammar-profile.v1.schema.json",
    "schemas/semantic-tree-sitter-query.v1.schema.json",
    "crates/agent-semantic-tree-sitter/src/query_syntax.rs",
    "crates/agent-semantic-client-core/src/request.rs",
    "packages/python/tools/src/tools/tree_sitter/contract.py",
)


def asp_tree_sitter_contract_fingerprint(
    extra_paths: Iterable[str] = (),
) -> str:
    digest = hashlib.sha256()
    for relative_path in sorted({*COMMON_CONTRACT_PATHS, *extra_paths}):
        path = REPO_ROOT / relative_path
        digest.update(relative_path.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return f"sha256:{digest.hexdigest()}"


def assert_asp_tree_sitter_contract(
    profile: dict[str, object],
    *,
    extra_paths: Iterable[str] = (),
) -> None:
    asp_workspace = profile["aspWorkspace"]
    assert isinstance(asp_workspace, dict)
    expected = asp_workspace["contractFingerprint"]
    assert isinstance(expected, str)
    actual = asp_tree_sitter_contract_fingerprint(extra_paths)
    assert expected == actual, (
        "aspWorkspace.contractFingerprint does not match the active ASP "
        f"tree-sitter contract: expected {expected}, actual {actual}"
    )


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        emit(
            "usage: python -m tools tree-sitter contract "
            "<grammar-profile.json> [extra-contract-path ...]",
            file=sys.stderr,
        )
        return 2
    profile = json.loads(Path(argv[1]).read_text())
    assert_asp_tree_sitter_contract(profile, extra_paths=argv[2:])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
