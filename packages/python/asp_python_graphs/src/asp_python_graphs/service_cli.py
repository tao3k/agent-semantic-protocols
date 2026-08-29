"""Run the ASP Server-owned Python Graphs gRPC service."""

from __future__ import annotations

import argparse
import asyncio
from collections.abc import Sequence
from pathlib import Path

from .grpc_service import DEFAULT_MAX_IN_FLIGHT, serve


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="asp-python-graphs serve")
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
