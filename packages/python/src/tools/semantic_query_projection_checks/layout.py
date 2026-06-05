"""Compact code text layout validators."""

from __future__ import annotations

import re
from collections.abc import Iterable

_LAYOUT_PUNCTUATION_ONLY = re.compile(r"^[\s\W_]+$")


def compact_code_layout_punctuation_errors(packet: dict[str, object]) -> list[str]:
    matches = packet.get("matches", [])
    if not isinstance(matches, Iterable):
        return []
    return [
        f"matches[{match_index}].code {error}"
        for match_index, match in enumerate(matches)
        if isinstance(match, dict) and _is_compact_projection(match.get("projection"))
        for code in [match.get("code")]
        if isinstance(code, str)
        for error in compact_code_text_layout_errors(
            code,
            allowed_punctuation_lines=_delimiter_row_line_numbers(
                match.get("projection")
            ),
        )
    ]


def compact_code_text_layout_errors(
    code: str,
    *,
    allowed_punctuation_lines: set[int] | None = None,
) -> list[str]:
    allowed_lines = allowed_punctuation_lines or set()
    return [
        f"line {line_number} is punctuation-only compact residue"
        for line_number, line in enumerate(code.splitlines(), start=1)
        if line_number not in allowed_lines and _LAYOUT_PUNCTUATION_ONLY.fullmatch(line)
    ]


def _delimiter_row_line_numbers(projection: object) -> set[int]:
    if not isinstance(projection, dict):
        return set()
    rows = projection.get("renderedRows")
    if not isinstance(rows, list):
        return set()
    return {
        row_index
        for row_index, row in enumerate(rows, start=1)
        if isinstance(row, dict) and row.get("rowKind") == "delimiter"
    }


def is_layout_punctuation_only(text: object) -> bool:
    return isinstance(text, str) and bool(_LAYOUT_PUNCTUATION_ONLY.fullmatch(text))


def _is_compact_projection(projection: object) -> bool:
    return isinstance(projection, dict) and projection.get("mode") == "compact"
