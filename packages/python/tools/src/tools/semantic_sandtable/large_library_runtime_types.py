"""Typed records shared by the large-library runtime benchmark seams."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Corpus:
    scenario_id: str
    language: str
    repository: str
    directory: str
    environment: str
    inputs: dict[str, str]


@dataclass(frozen=True, slots=True)
class Invocation:
    command: list[str]
    stdin: str | None
    expects_json: bool
    max_elapsed_ms: int


@dataclass(frozen=True, slots=True)
class CommandResult:
    returncode: int
    stdout: str
    stderr: str
    timed_out: bool
    process_tree_terminated: bool
