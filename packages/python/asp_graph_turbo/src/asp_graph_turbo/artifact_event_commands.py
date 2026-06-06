"""Provider command argv parsing for artifact event extraction."""

from __future__ import annotations


def artifact_command_argv(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str) and item)


def artifact_command_method(argv: tuple[str, ...]) -> str:
    search_index = _index_of(argv, "search")
    if search_index is not None:
        suffix, _ = _command_surface(argv, search_index + 1)
        return f"search/{suffix or 'unknown'}"
    query_index = _index_of(argv, "query")
    if query_index is not None:
        return "query"
    return "command/unknown"


def artifact_command_target(argv: tuple[str, ...]) -> str:
    search_index = _index_of(argv, "search")
    if search_index is not None:
        _, surface_index = _command_surface(argv, search_index + 1)
        return _next_positional(argv, surface_index + 1)
    query_index = _index_of(argv, "query")
    if query_index is not None:
        return _next_positional(argv, query_index + 1)
    return ""


def artifact_command_query(argv: tuple[str, ...]) -> str:
    search_index = _index_of(argv, "search")
    if search_index is None:
        return ""
    surface, surface_index = _command_surface(argv, search_index + 1)
    return _next_positional(argv, surface_index + 1) if surface == "fzf" else ""


def _next_positional(argv: tuple[str, ...], start: int) -> str:
    for item in argv[start:]:
        if item.startswith("-"):
            return ""
        return item
    return ""


def _command_surface(argv: tuple[str, ...], start: int) -> tuple[str, int]:
    index = start
    while index < len(argv):
        item = argv[index]
        if item == "--":
            index += 1
            continue
        if item.startswith("-"):
            index += 2 if item in _COMMAND_OPTIONS_WITH_VALUE else 1
            continue
        return item, index
    return "", len(argv)


def _index_of(items: tuple[str, ...], needle: str) -> int | None:
    return next((index for index, item in enumerate(items) if item == needle), None)


_COMMAND_OPTIONS_WITH_VALUE = {
    "--dependency",
    "--from-hook",
    "--format",
    "--owner",
    "--package",
    "--query",
    "--query-set",
    "--seeds",
    "--selector",
    "--view",
}
