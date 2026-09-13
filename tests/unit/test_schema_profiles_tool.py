# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""The Python schema-profile command is only a Rust CLI adapter."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/src"))

import tools.schema_profiles as schema_profiles  # noqa: E402


def test_schema_manager_argv_preserves_the_rust_authority_boundary(tmp_path: Path) -> None:
    assert schema_profiles.schema_manager_argv(
        "verify",
        ("rust", "gerbil-scheme"),
        repo_root=tmp_path,
        executable="schema-manager-fixture",
    ) == [
        "schema-manager-fixture",
        "verify",
        "--workspace",
        str(tmp_path),
        "--language",
        "rust",
        "--language",
        "gerbil-scheme",
    ]


def test_python_adapter_does_not_parse_or_mutate_schema_bundles(
    monkeypatch,
    tmp_path: Path,
) -> None:
    calls: list[tuple[list[str], bool]] = []

    def run(argv: list[str], *, check: bool) -> SimpleNamespace:
        calls.append((argv, check))
        return SimpleNamespace(returncode=17)

    monkeypatch.setattr(subprocess, "run", run)

    assert (
        schema_profiles.run_schema_manager(
            "materialize",
            ("gerbil-scheme",),
            repo_root=tmp_path,
            executable="schema-manager-fixture",
        )
        == 17
    )
    assert calls == [
        (
            [
                "schema-manager-fixture",
                "materialize",
                "--workspace",
                str(tmp_path),
                "--language",
                "gerbil-scheme",
            ],
            False,
        )
    ]
