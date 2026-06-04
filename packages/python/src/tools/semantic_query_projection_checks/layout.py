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
        for error in compact_code_text_layout_errors(code)
    ]


def compact_code_text_layout_errors(code: str) -> list[str]:
    return [
        f"line {line_number} is punctuation-only compact residue"
        for line_number, line in enumerate(code.splitlines(), start=1)
        if _LAYOUT_PUNCTUATION_ONLY.fullmatch(line)
    ]


def is_layout_punctuation_only(text: object) -> bool:
    return isinstance(text, str) and bool(_LAYOUT_PUNCTUATION_ONLY.fullmatch(text))


def _is_compact_projection(projection: object) -> bool:
    return isinstance(projection, dict) and projection.get("mode") == "compact"
