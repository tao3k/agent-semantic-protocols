# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Build complete, intent-bearing Search Playbook actions for schema fixtures."""

from __future__ import annotations


def complete_search_playbook_argv(
    *, language: str, term: str, path_hint: str, globs: tuple[str, ...]
) -> list[str]:
    rg_argv = ["--rg", "-n"]
    for glob in globs:
        rg_argv.extend(["-g", glob])
    rg_argv.extend(["-F", term, "."])
    return [
        "search",
        "playbook",
        "--language",
        language,
        *rg_argv,
        "--tantivy",
        f'title:"{path_hint}" OR body:"{term}"',
    ]


def complete_search_playbook_command(
    *, language: str, term: str, path_hint: str, globs: tuple[str, ...]
) -> dict[str, object]:
    return {
        "executable": "asp",
        "argv": complete_search_playbook_argv(
            language=language,
            term=term,
            path_hint=path_hint,
            globs=globs,
        ),
    }
