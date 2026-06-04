"""Projection identity and node uniqueness tests."""

from __future__ import annotations

import copy

from tools.semantic_query_projection import projection_uniqueness_errors

from .support import semantic_query_packet_with_projection


def test_projection_uniqueness_rejects_compact_exact_read_drift() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["exactRead"] = "src/chain.ts:1:7"

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert (
        "exactRead src/chain.ts:1:7 does not match read locator src/chain.ts:1:8"
        in errors
    )


def test_projection_uniqueness_rejects_unbound_source_fingerprint() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["sourceFingerprint"] = "sha256:abc123"

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert "sourceFingerprint does not include exactRead locator" in errors


def test_projection_uniqueness_rejects_duplicate_node_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["nodes"].append(
        {
            "id": "build:ret",
            "nativeId": "ts:return:duplicate",
            "kind": "return",
            "role": "terminal",
            "label": "return duplicated",
            "depth": 1,
            "read": "src/chain.ts:4:4",
            "structuralFingerprint": "return/duplicate",
        }
    )

    assert "duplicate node id build:ret" in "\n".join(
        projection_uniqueness_errors(packet)
    )


def test_projection_uniqueness_rejects_unknown_parent_and_rendered_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["nodes"][1]["parentId"] = "missing-parent"
    projection["renderedNodeIds"] = ["build", "missing-rendered"]

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert "missing parentId missing-parent" in errors
    assert "rendered node id missing-rendered does not exist" in errors


def test_projection_uniqueness_does_not_mutate_fixture() -> None:
    packet = semantic_query_packet_with_projection()
    cloned = copy.deepcopy(packet)

    assert projection_uniqueness_errors(packet) == []
    assert packet == cloned
