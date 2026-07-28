"""Typed records shared by the large-library runtime benchmark seams."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class LanguageExtensionAdmission:
    authority: str
    minimum_matching_files: int
    minimum_matching_file_ratio: float


@dataclass(frozen=True, slots=True)
class Corpus:
    resource_id: str
    scenario_id: str
    provider_id: str
    language: str
    repository: str
    remote: str
    revision: str
    directory: str
    environment: str
    inputs: dict[str, str]
    admission: LanguageExtensionAdmission | None = None


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
