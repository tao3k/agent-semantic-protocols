"""Projection layout residue tests for semantic query packets."""

from __future__ import annotations

from tools.semantic_query_projection import (
    compact_code_layout_punctuation_errors,
    projection_rendered_row_errors,
    projection_uniqueness_errors,
)

from .support import semantic_query_packet_with_projection, semantic_query_schema_validator


def test_projection_uniqueness_contract_accepts_canonical_packet() -> None:
    packet = semantic_query_packet_with_projection()

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))

    assert schema_errors == []
    assert projection_uniqueness_errors(packet) == []
    assert compact_code_layout_punctuation_errors(packet) == []


def test_compact_code_rejects_punctuation_only_lines() -> None:
    packet = semantic_query_packet_with_projection()
    packet["matches"][0]["code"] = "function build\nreturn map\n})"

    errors = "\n".join(
        [
            *compact_code_layout_punctuation_errors(packet),
            *projection_rendered_row_errors(packet),
        ]
    )

    assert "punctuation-only compact residue" in errors
    assert "renderedRows text does not match code" in errors


def test_projection_rendered_rows_reject_layout_residue() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    packet["matches"][0]["code"] = "function build\n}"
    projection["renderedRows"] = [
        {"nodeId": "build", "rowKind": "declaration", "text": "function build"},
        {"nodeId": "build:ret", "rowKind": "terminal", "text": "}"},
    ]

    errors = "\n".join(projection_rendered_row_errors(packet))

    assert "text is punctuation-only compact residue" in errors


def test_projection_rendered_rows_required_for_compact_code() -> None:
    packet = semantic_query_packet_with_projection()
    del packet["matches"][0]["projection"]["renderedRows"]

    errors = "\n".join(projection_rendered_row_errors(packet))

    assert "compact code lacks renderedRows" in errors
