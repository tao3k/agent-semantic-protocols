"""Patch safety schema tests for semantic query packets."""

from __future__ import annotations

from .support import semantic_query_minimal_packet, validation_errors


def test_compact_match_can_declare_patch_verify_safety() -> None:
    packet = semantic_query_minimal_packet()
    packet["patchSafety"] = {
        "level": "read-safe",
        "reason": "packet default requires exact source read before editing",
        "exactRead": "src/lib.rs:6:6",
    }
    packet["matches"][0]["patchSafety"] = {
        "level": "patch-verify-safe",
        "target": {
            "ownerPath": "src/lib.rs",
            "locator": "src/lib.rs#fn:load",
            "read": "src/lib.rs:6:6",
            "location": {"path": "src/lib.rs", "lineRange": "6:6"},
            "itemName": "load",
            "itemKind": "fn",
        },
        "preimageSource": "exact-read",
        "sourceFingerprint": "sha256:abc123",
        "parserVersion": "rust:rust-lang-project-harness",
        "allowedOperations": ["replace_statement", "append_to_block", "replace_item"],
        "losslessStructure": True,
    }

    assert validation_errors(packet) == []


def test_compact_match_can_declare_ast_patch_replace_item_safety() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["patchSafety"] = {
        "level": "ast-patch-safe",
        "target": {
            "ownerPath": "src/lib.rs",
            "locator": "src/lib.rs#fn:load",
            "read": "src/lib.rs:6:6",
            "location": {"path": "src/lib.rs", "lineRange": "6:6"},
            "itemName": "load",
            "itemKind": "fn",
        },
        "preimageSource": "exact-read",
        "sourceFingerprint": "src/lib.rs:6:6:39",
        "parserVersion": "rust:rs-harness",
        "allowedOperations": ["replace_item"],
        "losslessStructure": True,
        "notes": ["provider apply reparses and formats"],
    }

    assert validation_errors(packet) == []


def test_patch_verify_safety_requires_exact_read_preimage_source() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["patchSafety"] = {
        "level": "patch-verify-safe",
        "target": _patch_target(),
        "sourceFingerprint": "sha256:abc123",
        "parserVersion": "rust:rust-lang-project-harness",
        "allowedOperations": ["replace_statement"],
    }

    assert validation_errors(packet) != []


def test_patch_verify_safety_rejects_start_line_end_line() -> None:
    packet = semantic_query_minimal_packet()
    target = _patch_target()
    target["startLine"] = 6
    target["endLine"] = 6
    packet["matches"][0]["patchSafety"] = {
        "level": "patch-verify-safe",
        "target": target,
        "preimageSource": "exact-read",
        "sourceFingerprint": "sha256:abc123",
        "parserVersion": "rust:rust-lang-project-harness",
        "allowedOperations": ["replace_statement"],
    }

    assert validation_errors(packet) != []


def test_patch_verify_safety_requires_source_fingerprint() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["patchSafety"] = {
        "level": "patch-verify-safe",
        "target": _patch_target(),
        "parserVersion": "rust:rust-lang-project-harness",
        "allowedOperations": ["replace_statement"],
    }

    assert validation_errors(packet) != []


def _patch_target() -> dict[str, object]:
    return {
        "ownerPath": "src/lib.rs",
        "locator": "src/lib.rs#fn:load",
        "read": "src/lib.rs:6:6",
        "location": {"path": "src/lib.rs", "lineRange": "6:6"},
    }
