# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Private launcher for the ASP Server-owned Python Graphs gRPC service.

This module has one admitted operation, ``serve``.  It does not rank packets,
manage generations, or expose an algorithm command; those responsibilities
belong to the resident gRPC session and ASP Server.
"""

from __future__ import annotations

import argparse
import asyncio
from collections.abc import Sequence
from pathlib import Path

from .grpc_service import DEFAULT_MAX_IN_FLIGHT, serve


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="asp-python-graphs serve")
    parser.add_argument("command", choices=["serve"])
    parser.add_argument("--socket", required=True, type=Path)
    parser.add_argument(
        "--max-in-flight", type=int, default=DEFAULT_MAX_IN_FLIGHT
    )
    args = parser.parse_args(argv)
    if not args.socket.is_absolute():
        parser.error("--socket must be an absolute path")
    if args.max_in_flight < 1:
        parser.error("--max-in-flight must be positive")
    asyncio.run(serve(args.socket, args.max_in_flight))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
