"""Command tree for agent-semantic-protocols Python tooling."""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Literal, TextIO

from .console import emit


_ArgvMode = Literal["argv", "legacy_argv", "sys_argv", "no_args"]


@dataclass(frozen=True)
class CommandSpec:
    path: tuple[str, ...]
    module: str
    function: str
    argv_mode: _ArgvMode
    summary: str

    @property
    def display(self) -> str:
        return " ".join(self.path)

    @property
    def program(self) -> str:
        return f"python -m tools {self.display}"

    def run(self, argv: Sequence[str]) -> int:
        function = _load_function(self.module, self.function)
        match self.argv_mode:
            case "argv":
                result = function(list(argv))
            case "legacy_argv":
                result = function([self.program, *argv])
            case "sys_argv":
                result = _run_with_sys_argv(function, self.program, argv)
            case "no_args":
                result = self.run_no_args_command(function, argv)
            case _:  # pragma: no cover - typing keeps this unreachable.
                raise AssertionError(f"unknown argv mode: {self.argv_mode}")
        return int(result or 0)

    def run_no_args_command(
        self,
        function: Callable[[], object],
        argv: Sequence[str],
    ) -> object:
        if list(argv) in (["help"], ["--help"], ["-h"]):
            emit(f"usage: {self.program}")
            emit()
            emit(self.summary)
            return 0
        if argv:
            emit(
                f"{self.program}: unexpected arguments: {' '.join(argv)}",
                file=sys.stderr,
            )
            return 2
        return function()


COMMANDS: tuple[CommandSpec, ...] = (
    CommandSpec(
        ("sandtable",),
        "tools.semantic_sandtable.cli",
        "semantic_sandtable_main",
        "argv",
        "Run semantic sandtable scenarios and receipt checks.",
    ),
    CommandSpec(
        ("parser", "compact-snapshots"),
        "tools.parser_compact_snapshots",
        "main",
        "argv",
        "Print the retired root compact snapshot migration notice.",
    ),
    CommandSpec(
        ("codeql", "bounded-evidence"),
        "tools.codeql_bounded_evidence",
        "emit_codeql_bounded_evidence",
        "argv",
        "Emit bounded CodeQL evidence metadata for ASP flow fixtures.",
    ),
    CommandSpec(
        ("codeql", "evidence"),
        "tools.codeql_evidence",
        "emit_codeql_evidence",
        "argv",
        "Emit CodeQL CLI metadata as ASP evidence.",
    ),
    CommandSpec(
        ("graph", "turbo"),
        "asp_graph_turbo.cli",
        "main",
        "argv",
        "Rank typed ASP graph facts into compact frontier output.",
    ),
    CommandSpec(
        ("graph", "turbo", "artifacts"),
        "asp_graph_turbo.artifacts_cli",
        "main",
        "argv",
        "Evaluate graph turbo against cached ASP search artifacts.",
    ),
    CommandSpec(
        ("graph", "turbo", "timeline"),
        "asp_graph_turbo.timeline_cli",
        "main",
        "argv",
        "Infer search rounds and subagent microbursts from cached ASP artifacts.",
    ),
    CommandSpec(
        ("schema", "profiles"),
        "tools.schema_profiles",
        "main",
        "argv",
        "Validate language package schema downsync profiles.",
    ),
    CommandSpec(
        ("tree-sitter", "contract"),
        "tools.tree_sitter.contract",
        "main",
        "legacy_argv",
        "Validate a grammar-profile contract fingerprint.",
    ),
    CommandSpec(
        ("tree-sitter", "validate", "json-abi-corpus"),
        "tools.tree_sitter.validate_json_abi_corpus",
        "main",
        "no_args",
        "Validate tree-sitter JSON ABI corpus capture output.",
    ),
    CommandSpec(
        ("tree-sitter", "validate", "python-query-corpus"),
        "tools.tree_sitter.validate_python_query_corpus",
        "main",
        "no_args",
        "Validate Python tree-sitter query corpus fixtures.",
    ),
    CommandSpec(
        ("tree-sitter", "validate", "rust-query-corpus"),
        "tools.tree_sitter.validate_rust_query_corpus",
        "main",
        "sys_argv",
        "Validate Rust tree-sitter query corpus fixtures.",
    ),
    CommandSpec(
        ("tree-sitter", "validate", "typescript-query-corpus"),
        "tools.tree_sitter.validate_typescript_query_corpus",
        "main",
        "sys_argv",
        "Validate TypeScript tree-sitter query corpus fixtures.",
    ),
    CommandSpec(
        ("tree-sitter", "sync", "rust-queries"),
        "tools.tree_sitter.sync_rust_queries",
        "main",
        "sys_argv",
        "Sync Rust tree-sitter query snapshots from an upstream checkout.",
    ),
    CommandSpec(
        ("tree-sitter", "sync", "typescript-query-corpus"),
        "tools.tree_sitter.sync_typescript_query_corpus",
        "main",
        "sys_argv",
        "Refresh TypeScript tree-sitter query corpus metadata.",
    ),
)


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        _print_help(())
        return 2
    if args == ["help"] or args == ["--help"] or args == ["-h"]:
        _print_help(())
        return 0

    command = _match_command(args)
    if command is not None:
        return command.run(args[len(command.path) :])

    if args[-1] in {"help", "--help", "-h"}:
        prefix = tuple(args[:-1])
        if _commands_under(prefix):
            _print_help(prefix)
            return 0

    emit(f"python -m tools: unknown command: {' '.join(args)}", file=sys.stderr)
    _print_help((), file=sys.stderr)
    return 2


def _match_command(args: Sequence[str]) -> CommandSpec | None:
    for command in sorted(COMMANDS, key=lambda item: len(item.path), reverse=True):
        if tuple(args[: len(command.path)]) == command.path:
            return command
    return None


def _commands_under(prefix: tuple[str, ...]) -> tuple[CommandSpec, ...]:
    return tuple(command for command in COMMANDS if command.path[: len(prefix)] == prefix)


def _print_help(prefix: tuple[str, ...], *, file: TextIO = sys.stdout) -> None:
    commands = _commands_under(prefix)
    if prefix:
        header = f"usage: python -m tools {' '.join(prefix)} <command> [args]"
    else:
        header = "usage: python -m tools <command> [args]"
    emit(header, file=file)
    emit(file=file)
    emit("commands:", file=file)
    for command in commands:
        emit(f"  {command.display:<46} {command.summary}", file=file)


def _load_function(module_name: str, function_name: str) -> Callable[..., object]:
    module = importlib.import_module(module_name)
    function = getattr(module, function_name)
    if not callable(function):
        raise TypeError(f"{module_name}:{function_name} is not callable")
    return function


def _run_with_sys_argv(
    function: Callable[..., object],
    program: str,
    argv: Sequence[str],
) -> object:
    original_argv = sys.argv[:]
    sys.argv = [program, *argv]
    try:
        return function()
    finally:
        sys.argv = original_argv
