"""Compact projection metadata schema tests."""

from __future__ import annotations

from .support import semantic_query_minimal_packet, validation_errors


def test_compact_projection_requires_navigation_metadata() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = {
        "mode": "compact",
        "syntax": "semantic-outline",
        "sourceAuthority": "native-parser",
    }

    assert validation_errors(packet) != []


def test_save_token_ruff_projection_requires_formatter_structural_safety() -> None:
    packet = semantic_query_minimal_packet()
    projection = packet["matches"][0]["projection"]
    projection["syntax"] = "save-token-ruff"
    projection["compactSafety"]["whitespacePolicy"] = "semantic-outline"

    assert validation_errors(packet) != []


def test_save_token_projection_cannot_be_outline_mode() -> None:
    packet = semantic_query_minimal_packet()
    projection = packet["matches"][0]["projection"]
    projection["syntax"] = "save-token-ruff"
    projection["mode"] = "outline"

    assert validation_errors(packet) != []


def test_save_token_projection_requires_exact_read_before_editing() -> None:
    packet = semantic_query_minimal_packet()
    projection = packet["matches"][0]["projection"]
    projection["syntax"] = "save-token-ruff"
    projection["compactSafety"]["exactReadRequired"] = False

    assert validation_errors(packet) != []


def test_compact_projection_node_requires_native_identity_metadata() -> None:
    for field_name in ("nativeId", "structuralFingerprint"):
        packet = semantic_query_minimal_packet()
        node = packet["matches"][0]["projection"]["nodes"][0]
        del node[field_name]

        assert validation_errors(packet) != []


def test_source_code_match_accepts_content_compaction_policy() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["contentKind"] = "source-code"
    packet["matches"][0]["criticality"] = "exact-source-required-for-edit"
    packet["matches"][0]["compaction"] = {
        "mode": "source-code-call-skeleton",
        "lossiness": "bounded",
        "trustLevel": "parser-backed",
        "sourceOfTruth": "parser-facts",
        "validFor": ["navigation", "reasoning"],
        "notValidFor": ["patch", "line-edit", "exact-source"],
        "preserved": ["signature", "called-symbols", "return-points"],
        "omitted": ["large-literals", "private-branch-body"],
        "requiresExactSourceFor": ["patch", "compile-fix"],
        "exactSourceRequired": True,
    }

    assert validation_errors(packet) == []
