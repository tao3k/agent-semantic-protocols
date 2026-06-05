"""Semantic query projection contract tests."""

from __future__ import annotations

from .support import (
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
