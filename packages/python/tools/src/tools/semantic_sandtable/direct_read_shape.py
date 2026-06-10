"""Classify direct-source-read commands by selector span."""

from __future__ import annotations

DIRECT_READ_BOUNDED_MAX_LINES = 80


def direct_source_read_shape(args: list[str]) -> str:
    selector = option_value(args, "--selector")
    line_span = selector_line_span(selector)
    if line_span is None:
        return "unbounded"
    if line_span <= DIRECT_READ_BOUNDED_MAX_LINES:
        return "bounded"
    return "broad"


def option_value(args: list[str], option: str) -> str | None:
    for index, arg in enumerate(args):
        if arg == option:
            if index + 1 < len(args):
                return args[index + 1]
            return None
        prefix = f"{option}="
        if arg.startswith(prefix):
            return arg[len(prefix) :]
    return None


def selector_line_span(selector: str | None) -> int | None:
    if not selector:
        return None
    suffix = selector.rsplit(":", maxsplit=1)[-1]
    if "-" in suffix:
        return _line_span_from_parts(suffix.split("-", maxsplit=1))
    parts = selector.rsplit(":", maxsplit=2)
    if len(parts) >= 3 and parts[-2].isdigit() and parts[-1].isdigit():
        return _line_span(int(parts[-2]), int(parts[-1]))
    if suffix.isdigit():
        return 1
    return None


def _line_span_from_parts(parts: list[str]) -> int | None:
    if len(parts) != 2 or not parts[0].isdigit() or not parts[1].isdigit():
        return None
    return _line_span(int(parts[0]), int(parts[1]))


def _line_span(start: int, end: int) -> int | None:
    if start <= 0 or end <= 0 or end < start:
        return None
    return end - start + 1
