"""Structural-index schema contract tests."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .schema_validation import schema_validator_for


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _ROOT / "schemas" / "semantic-structural-index.v1.schema.json"


def structural_index_packet() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": "rust-main-1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": "/tmp/example",
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": "structural-index/rust-main-1.json",
        "indexFingerprint": "fnv64:0123456789abcdef",
        "rawSourceStored": False,
        "fileHashes": [
            {
                "path": "src/lib.rs",
                "sha256": "0" * 64,
                "source": "provider",
            },
            {
                "path": "Cargo.lock",
                "sha256": "1" * 64,
                "source": "lockfile",
            },
        ],
        "owners": [
            {
                "ownerPath": "src/lib.rs",
                "ownerKind": "source",
                "sourceAuthority": "native-parser",
                "location": {"path": "src/lib.rs", "lineRange": "1:40"},
                "queryKeys": ["config", "parse_config"],
            }
        ],
        "symbols": [
            {
                "ownerPath": "src/lib.rs",
                "name": "parse_config",
                "qualifiedName": "example::parse_config",
                "kind": "function",
                "visibility": "public",
                "sourceLocator": "src/lib.rs:3:12",
                "nativeFactRefs": ["rust:item:src/lib.rs:3:12:parse_config"],
                "queryKeys": ["parse", "config", "parse_config"],
            }
        ],
        "dependencyUsages": [
            {
                "ownerPath": "src/lib.rs",
                "packageName": "serde_json",
                "packageVersion": "1.0.0",
                "apiName": "from_str",
                "importPath": "serde_json::from_str",
                "manifestPath": "Cargo.toml",
                "lockfileHash": "sha256:" + "1" * 64,
                "source": "manifest+native-parser",
                "sourceLocator": "src/lib.rs:8:8",
                "queryKeys": ["serde_json", "serde_json::from_str", "json parse"],
            }
        ],
    }


def structural_index_validation_errors(packet: dict[str, Any]) -> list[str]:
    validator = schema_validator_for(_SCHEMA_PATH)
    return [error.message for error in validator.iter_errors(packet)]


def test_semantic_structural_index_accepts_structural_cache_rows() -> None:
    assert structural_index_validation_errors(structural_index_packet()) == []


def test_semantic_structural_index_rejects_raw_source_storage() -> None:
    packet = structural_index_packet()
    packet["rawSourceStored"] = True

    assert any(
        "False was expected" in error
        for error in structural_index_validation_errors(packet)
    )


def test_semantic_structural_index_rejects_source_payload_fields() -> None:
    packet = structural_index_packet()
    packet["symbols"][0]["sourceText"] = "pub fn parse_config() {}"

    assert any(
        "Additional properties are not allowed" in error
        for error in structural_index_validation_errors(packet)
    )
