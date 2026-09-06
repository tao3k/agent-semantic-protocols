# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Command tree for agent-semantic-protocols Python tooling."""

from __future__ import annotations

import sys
from collections.abc import Sequence

from .command_dispatch import commands_under, match_command, print_help
from .console import emit


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        print_help(())
        return 2
    if args == ["help"] or args == ["--help"] or args == ["-h"]:
        print_help(())
        return 0

    command = match_command(args)
    if command is not None:
        return command.run(args[len(command.path) :])

    if args[-1] in {"help", "--help", "-h"}:
        prefix = tuple(args[:-1])
        if commands_under(prefix):
            print_help(prefix)
            return 0

    emit(f"python -m tools: unknown command: {' '.join(args)}", file=sys.stderr)
    print_help((), file=sys.stderr)
    return 2
