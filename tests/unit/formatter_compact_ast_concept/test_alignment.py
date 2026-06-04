"""Formatter alignment tests for compact AST projection."""

from __future__ import annotations

from .support import AlignmentResult, compact_projection, formatter_normalized_compact


def test_formatter_style_is_opaque_when_native_ast_aligns() -> None:
    original = """
def score( x:int):
    if (x> 0):
       return x+1
    return 0
"""
    style_a = """
def score(x: int):
    if x > 0:
        return x + 1
    return 0
"""
    style_b = """
def score(
    x: int,
):
    if x > 0:
        return x + 1
    return 0
"""

    assert formatter_normalized_compact(original, style_a) == AlignmentResult(
        status="ok",
        projection_mode="formatter-normalized",
    )
    assert formatter_normalized_compact(original, style_b) == AlignmentResult(
        status="ok",
        projection_mode="formatter-normalized",
    )
    assert compact_projection(style_a) == compact_projection(style_b)
    assert compact_projection(style_a) == (
        "def score(x: int)",
        "if x > 0",
        "return x + 1",
        "return 0",
    )


def test_formatter_alignment_failure_is_not_a_fallback_path() -> None:
    original = """
def score(x: int):
    if x > 0:
        return x + 1
    return 0
"""
    semantically_changed = """
def score(x: int):
    if x > 0:
        return x + 2
    return 0
"""

    result = formatter_normalized_compact(original, semantically_changed)

    assert result == AlignmentResult(
        status="failed",
        projection_mode="formatter-normalized",
        failure_kind="formatter-alignment-failed",
    )
