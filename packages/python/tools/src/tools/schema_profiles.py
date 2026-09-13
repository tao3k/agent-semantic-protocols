# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Thin command adapter for the Rust-owned ASP Schema Manager.

The Rust ``agent-semantic-schema-manager`` binary is the sole authority for
profile closure, bundle mutation, receipts, and verification. Python never
recomputes membership and never copies or removes schema files.
"""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path
from typing import Sequence


def _find_repo_root() -> Path:
    for candidate in Path(__file__).resolve().parents:
        if (candidate / "schemas" / "language-schema-profiles.json").is_file():
            return candidate
    raise RuntimeError("unable to locate ASP repository root")


REPO_ROOT = _find_repo_root()


def schema_manager_argv(
    command: str,
    languages: Sequence[str],
    *,
    repo_root: Path = REPO_ROOT,
    executable: str | None = None,
) -> list[str]:
    """Build the exact Rust Schema Manager invocation without interpreting it."""

    manager = executable or os.environ.get("ASP_SCHEMA_MANAGER_BIN", "asp-schema-manager")
    argv = [manager, command, "--workspace", str(repo_root)]
    for language in languages:
        argv.extend(("--language", language))
    return argv


def run_schema_manager(
    command: str,
    languages: Sequence[str] = (),
    *,
    repo_root: Path = REPO_ROOT,
    executable: str | None = None,
) -> int:
    """Delegate unchanged stdio and exit status to the Rust authority."""

    completed = subprocess.run(
        schema_manager_argv(
            command,
            languages,
            repo_root=repo_root,
            executable=executable,
        ),
        check=False,
    )
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="schema-profiles")
    parser.add_argument("command", choices=("materialize", "verify"))
    parser.add_argument("languages", nargs="*")
    args = parser.parse_args(argv)
    return run_schema_manager(args.command, args.languages)
