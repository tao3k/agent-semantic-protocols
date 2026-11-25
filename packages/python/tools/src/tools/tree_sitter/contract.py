# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

ACTIVE_ABI_PATHS = (
    "schemas/semantic-tree-sitter-grammar-profile.v1.schema.json",
    "schemas/semantic-tree-sitter-query.v1.schema.json",
    "crates/agent-semantic-tree-sitter/src/query_syntax.rs",
)

COMMON_CONTRACT_PATHS = (
    *ACTIVE_ABI_PATHS,
    "packages/python/tools/src/tools/tree_sitter/contract.py",
)

# Pinned external provider repos (providers.toml) were stamped at this ASP revision
# before `crates/agent-semantic-client-core` was retired in RFC 10.34. Until those
# external repos re-stamp against the slimmed ASP main, accept their audited
# fingerprints strictly while ACTIVE_ABI_PATHS remain byte-identical.
_PINNED_PRE_SLIMMING_REVISION = "4e3417721d576716bcf82b7cbf8b9c2dc9a2b32a"
_PINNED_ACTIVE_ABI_DIGEST = (
    "sha256:e5753caee76402e50bae3b2a5b084b528d366efcc9c29972c76d9f0c16cb2743"
)
_PINNED_PROVIDER_FINGERPRINTS = frozenset(
    {
        "sha256:183eb9b0c727f815e67e46e96b00d832840afc6ae7ec16ad726cd10e797e0bbf",
        "sha256:62942e035cae84e6918a76955ba2269784843d21f5e5cb4e284242b48cb9e8d2",
        "sha256:403556a55568509092dcb59625e0782614f3e0a42540996f64c15c2f3903d419",
        "sha256:bb4278b3d1d3bc44bd6ba8a3a1dd3fb9470c9fcd5d42a842e3427d32c31cf2f6",
    }
)


def _hash_paths(paths: Iterable[str]) -> str:
    digest = hashlib.sha256()
    for relative_path in sorted(set(paths)):
        path = REPO_ROOT / relative_path
        digest.update(relative_path.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return f"sha256:{digest.hexdigest()}"


def asp_tree_sitter_contract_fingerprint(
    extra_paths: Iterable[str] = (),
) -> str:
    return _hash_paths((*COMMON_CONTRACT_PATHS, *extra_paths))


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
    if expected == actual:
        return
    if (
        asp_workspace.get("revision") == _PINNED_PRE_SLIMMING_REVISION
        and expected in _PINNED_PROVIDER_FINGERPRINTS
        and _hash_paths(ACTIVE_ABI_PATHS) == _PINNED_ACTIVE_ABI_DIGEST
    ):
        return
    raise AssertionError(
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
