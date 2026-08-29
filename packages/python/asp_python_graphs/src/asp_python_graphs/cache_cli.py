"""Cache maintenance CLI for ASP graph turbo."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence

from .cache_store import graph_cache_status, invalidate_graph_cache, prune_graph_cache


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    if args.command == "status":
        _write_status(args.format)
        return 0
    if args.command == "invalidate":
        removed = invalidate_graph_cache()
        _write_mutation("invalidate", removed, args.format)
        return 0
    if args.command == "prune":
        removed = prune_graph_cache(args.max_entries)
        _write_mutation("prune", removed, args.format)
        return 0
    raise AssertionError(f"unhandled cache command: {args.command}")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("status", "invalidate"):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--format", choices=["text", "json"], default="text")
    prune = subparsers.add_parser("prune")
    prune.add_argument("--max-entries", type=int, default=16)
    prune.add_argument("--format", choices=["text", "json"], default="text")
    return parser.parse_args(argv)


def _write_status(output_format: str) -> None:
    status = graph_cache_status()
    if output_format == "json":
        sys.stdout.write(json.dumps(status, sort_keys=True) + "\n")
        return
    sys.stdout.write(
        "[graph-turbo-cache] "
        f"enabled={str(status['enabled']).lower()} "
        f"entries={status['entries']} "
        f"bytes={status['bytes']} "
        f"memoryEntries={status['memoryEntries']} "
        f"root={status['root']}\n"
    )


def _write_mutation(action: str, removed: int, output_format: str) -> None:
    status = graph_cache_status()
    payload = {"action": action, "removed": removed, **status}
    if output_format == "json":
        sys.stdout.write(json.dumps(payload, sort_keys=True) + "\n")
        return
    sys.stdout.write(
        f"[graph-turbo-cache] action={action} removed={removed} "
        f"entries={status['entries']} root={status['root']}\n"
    )
