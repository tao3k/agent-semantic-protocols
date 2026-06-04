"""Projection reverse-navigation action tests."""

from __future__ import annotations

from tools.semantic_query_projection import projection_uniqueness_errors

from .support import semantic_query_packet_with_projection, semantic_query_schema_validator


def test_projection_uniqueness_rejects_unanchored_omitted_facts() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["omitted"] = [
        {"kind": "body-detail", "reason": "hidden without reverse navigation"}
    ]

    assert "omitted fact lacks nodeId/read" in "\n".join(
        projection_uniqueness_errors(packet)
    )


def test_projection_uniqueness_rejects_node_query_without_node_target() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["expandActions"] = [
        {
            "kind": "node-query",
            "target": "src/chain.ts:2:7",
            "argv": ["ts-harness", "search", "owner", "src/chain.ts", "items", "."],
            "reason": "node query must target a projection node, not a read locator",
        }
    ]

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))
    errors = "\n".join(projection_uniqueness_errors(packet))

    assert schema_errors == []
    assert "node-query target src/chain.ts:2:7 does not exist" in errors


def test_projection_uniqueness_rejects_exact_read_argv_selector_drift() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["expandActions"] = [
        {
            "kind": "exact-read",
            "target": "build:ret",
            "read": "src/chain.ts:2:7",
            "argv": [
                "ts-harness",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "src/chain.ts",
                ".",
            ],
            "reason": "exact read argv must use the same read locator",
        }
    ]

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))
    errors = "\n".join(projection_uniqueness_errors(packet))

    assert schema_errors == []
    assert (
        "argv selector src/chain.ts does not match read locator src/chain.ts:2:7"
        in errors
    )
