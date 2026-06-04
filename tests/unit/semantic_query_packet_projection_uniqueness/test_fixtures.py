"""Parser compact fixture projection contract tests."""

from __future__ import annotations

import json

from tools.semantic_query_projection import (
    compact_code_layout_punctuation_errors,
    projection_uniqueness_errors,
)

from .support import (
    parser_compact_query_fixture_paths,
    repo_relative_path,
    semantic_query_packet_with_projection,
    semantic_query_schema_validator,
)


def test_schema_rejects_duplicate_rendered_node_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["renderedNodeIds"] = ["build", "build"]

    messages = [
        error.message for error in semantic_query_schema_validator().iter_errors(packet)
    ]

    assert any("non-unique elements" in message for message in messages)


def test_parser_compact_query_fixtures_obey_projection_contract() -> None:
    fixture_paths = parser_compact_query_fixture_paths()

    assert fixture_paths

    validator = semantic_query_schema_validator()
    failures: list[str] = []
    for fixture_path in fixture_paths:
        packet = json.loads(fixture_path.read_text(encoding="utf-8"))
        relative_path = repo_relative_path(fixture_path)
        failures.extend(
            f"{relative_path}: schema: {error.message}"
            for error in validator.iter_errors(packet)
        )
        failures.extend(
            f"{relative_path}: projection: {error}"
            for error in projection_uniqueness_errors(packet)
        )
        failures.extend(
            f"{relative_path}: compact-code: {error}"
            for error in compact_code_layout_punctuation_errors(packet)
        )

    assert failures == []
