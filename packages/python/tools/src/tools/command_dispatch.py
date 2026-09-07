# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Dispatch helpers for the Python tooling command tree."""

from __future__ import annotations

import sys
from collections.abc import Sequence
from typing import TextIO

from .command_catalog import COMMANDS
from .command_spec import CommandSpec
from .console import emit


def match_command(args: Sequence[str]) -> CommandSpec | None:
    for command in sorted(COMMANDS, key=lambda item: len(item.path), reverse=True):
        if tuple(args[: len(command.path)]) == command.path:
            return command
    return None


def commands_under(prefix: tuple[str, ...]) -> tuple[CommandSpec, ...]:
    return tuple(command for command in COMMANDS if command.path[: len(prefix)] == prefix)


def print_help(prefix: tuple[str, ...], *, file: TextIO = sys.stdout) -> None:
    commands = commands_under(prefix)
    if prefix:
        header = f"usage: python -m tools {' '.join(prefix)} <command> [args]"
    else:
        header = "usage: python -m tools <command> [args]"
    emit(header, file=file)
    emit(file=file)
    emit("commands:", file=file)
    for command in commands:
        emit(f"  {command.display:<46} {command.summary}", file=file)
