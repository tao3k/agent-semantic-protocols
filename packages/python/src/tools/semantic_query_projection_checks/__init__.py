"""Focused validators for semantic query projection packets."""

from __future__ import annotations

from .layout import (
    compact_code_layout_punctuation_errors,
    compact_code_text_layout_errors,
)
from .rendered_rows import projection_rendered_row_errors
from .uniqueness import projection_uniqueness_errors

__all__ = [
    "compact_code_layout_punctuation_errors",
    "compact_code_text_layout_errors",
    "projection_rendered_row_errors",
    "projection_uniqueness_errors",
    "semantic_query_projection_errors",
]


def semantic_query_projection_errors(packet: dict[str, object]) -> list[str]:
    return [
        *projection_uniqueness_errors(packet),
        *projection_rendered_row_errors(packet),
        *compact_code_layout_punctuation_errors(packet),
    ]
