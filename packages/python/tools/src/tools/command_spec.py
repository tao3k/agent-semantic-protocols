"""Executable command specification for the Python tooling tree."""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Literal

from .console import emit


ArgvMode = Literal["argv", "retired_argv", "sys_argv", "no_args"]


@dataclass(frozen=True)
class CommandSpec:
    path: tuple[str, ...]
    module: str
    function: str
    argv_mode: ArgvMode
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
            case "retired_argv":
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
