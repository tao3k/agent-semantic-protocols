"""Provider command artifact labels for graph turbo evaluation."""

from __future__ import annotations

import json
import shlex
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class CommandLabel:
    language: str
    kind: str
    target: str
    argv: tuple[str, ...]
    path: str


def command_artifact_dir(root: Path) -> Path | None:
    if root.name == "prompt-output":
        return root
    if root.name == "search":
        candidate = root.parent / "prompt-output"
        return candidate if candidate.is_dir() else None
    candidate = root / "prompt-output"
    return candidate if candidate.is_dir() else None


def command_labels_by_language(root: Path | None) -> tuple[CommandLabel, ...]:
    return tuple(
        label
        for path in _command_artifact_paths(root)
        for label in _labels_from_packet(path)
    )


def shell_command(argv: Sequence[str]) -> str:
    return " ".join(shlex.quote(item) for item in argv if item)


def target_like(value: str) -> bool:
    return _target_like(value)


def _command_artifact_paths(root: Path | None) -> tuple[Path, ...]:
    if root is None or not root.is_dir():
        return ()
    return tuple(sorted(path for path in root.iterdir() if path.name.endswith(".command.json")))


def _labels_from_packet(path: Path) -> tuple[CommandLabel, ...]:
    commands = _load_json(path).get("providerCommands")
    if not isinstance(commands, list):
        return ()
    return tuple(
        label
        for command in commands
        for label in _labels_from_command(command, path)
    )


def _load_json(path: Path) -> Mapping[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    return value if isinstance(value, Mapping) else {}


def _labels_from_command(command: object, path: Path) -> tuple[CommandLabel, ...]:
    if not isinstance(command, Mapping):
        return ()
    argv = _argv(command.get("argv"))
    language = str(command.get("languageId") or "unknown")
    if not argv:
        return ()
    search_index = _index_of(argv, "search")
    if search_index is not None:
        return _search_labels(argv, search_index, language, path)
    query_index = _index_of(argv, "query")
    if query_index is not None:
        return _single_target_label(argv, query_index + 1, language, "selector", path)
    return ()


def _argv(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(str(item) for item in value if isinstance(item, str) and item)


def _search_labels(
    argv: tuple[str, ...], index: int, language: str, path: Path
) -> tuple[CommandLabel, ...]:
    if index + 1 >= len(argv):
        return ()
    command = argv[index + 1]
    if command == "owner":
        return _single_target_label(argv, index + 2, language, "owner", path)
    if command == "tests":
        return _single_target_label(argv, index + 2, language, "test", path)
    if command == "query":
        return _single_target_label(argv, index + 2, language, "selector", path)
    if command in {"deps", "dependency"}:
        return _single_target_label(argv, index + 2, language, "dependency", path)
    return ()


def _single_target_label(
    argv: tuple[str, ...], start: int, language: str, kind: str, path: Path
) -> tuple[CommandLabel, ...]:
    target = _next_positional(argv, start)
    if target is None or not _target_like(target):
        return ()
    return (
        CommandLabel(
            language=language,
            kind=kind,
            target=target,
            argv=argv,
            path=str(path),
        ),
    )


def _next_positional(argv: tuple[str, ...], start: int) -> str | None:
    return next((item for item in argv[start:] if not item.startswith("-")), None)


def _target_like(value: str) -> bool:
    suffixes = (
        ".rs",
        ".py",
        ".ts",
        ".tsx",
        ".js",
        ".jl",
        ".toml",
        ".org",
        ".md",
    )
    return (
        "/" in value
        or "\\" in value
        or ":" in value
        or value.startswith(".")
        or value.endswith(suffixes)
    )


def _index_of(items: Sequence[str], needle: str) -> int | None:
    return next((index for index, item in enumerate(items) if item == needle), None)
