# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import os
import subprocess
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PROFILE_EXEC = REPOSITORY_ROOT / "scripts" / "devenv-profile-exec.sh"


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)


def test_profile_exec_evaluates_once_and_refreshes_only_on_input_drift(
    tmp_path: Path,
) -> None:
    root = tmp_path / "workspace"
    state = tmp_path / "state"
    fake_bin = tmp_path / "bin"
    profile = tmp_path / "devenv-profile"
    calls = tmp_path / "direnv.calls"
    root.mkdir()
    fake_bin.mkdir()
    (profile / "bin").mkdir(parents=True)
    (root / ".envrc").write_text("use devenv\n", encoding="utf-8")
    (root / "devenv.nix").write_text("{ packages = []; }\n", encoding="utf-8")

    _write_executable(
        fake_bin / "direnv",
        """#!/bin/sh
set -eu
printf 'call\\n' >> "$FAKE_DIRENV_CALLS"
test "$1" = exec
shift
shift
DEVENV_PROFILE="$FAKE_DEVENV_PROFILE" exec "$@"
""",
    )
    _write_executable(
        profile / "bin" / "profile-probe",
        """#!/bin/sh
set -eu
printf 'profile=%s\\narg=%s\\n' "$DEVENV_PROFILE" "$1"
""",
    )

    env = os.environ.copy()
    env.update(
        {
            "ASP_DEVENV_EXEC_ROOT": str(root),
            "ASP_DEVENV_EXEC_STATE": str(state),
            "ASP_DEVENV_EXEC_ALLOW_NON_STORE_PROFILE": "1",
            "FAKE_DEVENV_PROFILE": str(profile),
            "FAKE_DIRENV_CALLS": str(calls),
            "PATH": f"{fake_bin}:{env['PATH']}",
        }
    )

    def run() -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(PROFILE_EXEC), "profile-probe", "space preserved"],
            cwd=root,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )

    first = run()
    assert first.returncode == 0, first.stderr
    assert first.stdout == f"profile={profile}\narg=space preserved\n"
    assert calls.read_text(encoding="utf-8") == "call\n"

    second = run()
    assert second.returncode == 0, second.stderr
    assert calls.read_text(encoding="utf-8") == "call\n"

    (root / "devenv.nix").write_text("{ packages = [ 1 ]; }\n", encoding="utf-8")
    third = run()
    assert third.returncode == 0, third.stderr
    assert calls.read_text(encoding="utf-8") == "call\ncall\n"

    cache = state / "asp-profile-exec" / "profile.v1"
    assert cache.stat().st_mode & 0o777 == 0o600
