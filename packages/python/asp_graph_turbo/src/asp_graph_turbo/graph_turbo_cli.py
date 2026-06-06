"""Explicit graph-turbo command dispatcher."""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable, Sequence


_COMMANDS: dict[str, tuple[str, str, str]] = {
    "rank": (
        "asp_graph_turbo.cli",
        "main",
        "Rank a graph turbo request packet into compact frontier output.",
    ),
    "artifacts": (
        "asp_graph_turbo.artifacts_cli",
        "main",
        "Evaluate graph turbo against cached ASP search artifacts.",
    ),
    "timeline": (
        "asp_graph_turbo.timeline_cli",
        "main",
        "Audit cached ASP artifacts as timeline, episode, and frontier actions.",
    ),
}


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args or args[0] in {"help", "--help", "-h"}:
        _print_help()
        return 0 if args else 2
    command = args[0]
    spec = _COMMANDS.get(command)
    if spec is None:
        sys.stderr.write(f"graph-turbo: unknown command: {command}\n")
        _print_help(file=sys.stderr)
        return 2
    module_name, function_name, _ = spec
    return int(_load_function(module_name, function_name)(args[1:]) or 0)


def _load_function(module_name: str, function_name: str) -> Callable[..., object]:
    module = importlib.import_module(module_name)
    function = getattr(module, function_name)
    if not callable(function):
        raise TypeError(f"{module_name}:{function_name} is not callable")
    return function


def _print_help(*, file: object | None = None) -> None:
    output = sys.stdout if file is None else file
    output.write("usage: graph-turbo <command> [args]\n\n")
    output.write("commands:\n")
    for name, (_, _, summary) in sorted(_COMMANDS.items()):
        output.write(f"  {name:<12} {summary}\n")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
