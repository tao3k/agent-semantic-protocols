#!/usr/bin/env python3
# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Sync Rust tree-sitter query snapshots from an upstream checkout."""

from __future__ import annotations

import sys

from tools.paths import repo_root
from tools.tree_sitter.sync_query_snapshots import main as sync_query_snapshots_main


DEFAULT_PROVIDER_DIR = (
    repo_root() / "languages/asp-rust/tree-sitter/tree-sitter-rust"
)


def main() -> int:
    return sync_query_snapshots_main(
        list(sys.argv[1:]),
        default_provider_dir=DEFAULT_PROVIDER_DIR,
        output_label="Rust query",
    )


if __name__ == "__main__":
    raise SystemExit(main())
