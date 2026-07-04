"""Schema tests for Merkle artifact fields in client receipts."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def artifact_hash(value: str = "a") -> dict[str, str]:
    return {
        "algorithm": "blake3",
        "value": value * 64,
    }


def artifact_root(root_kind: str = "searchReceipt") -> dict[str, Any]:
    return {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "g1",
        "rootKind": root_kind,
        "rootHash": artifact_hash("b"),
        "nodeHash": artifact_hash("c"),
        "producerHash": artifact_hash("d"),
        "schemaHash": artifact_hash("e"),
        "contentHash": artifact_hash("f"),
    }


def receipt_schema_validator() -> Draft202012Validator:
    schema_path = _REPO_ROOT / "schemas" / "agent-semantic-client-receipt.v1.schema.json"
    with schema_path.open("r", encoding="utf-8") as handle:
        return Draft202012Validator(json.load(handle))


def validation_errors(receipt: dict[str, Any]) -> list[str]:
    validator = receipt_schema_validator()
    return [error.message for error in validator.iter_errors(receipt)]


def test_merkle_artifact_root_receipt_fields_are_valid() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.client-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "method": "search",
        "route": "local-native",
        "cacheStatus": "miss",
        "providerCommandCount": 1,
        "providerProcessesSpawned": 1,
        "providerCommands": [
            {
                "languageId": "rust",
                "providerId": "rs-harness",
                "argv": ["rs-harness", "search", "prime", "."],
                "exitCode": 0,
                "stdoutBytes": 300,
                "stderrBytes": 0,
                "elapsedMs": 12,
                "stdoutBlake3": artifact_hash("1"),
                "stderrBlake3": artifact_hash("2"),
            }
        ],
        "nativeProvenance": [],
        "artifactRoots": [
            artifact_root("searchReceipt"),
            artifact_root("providerOutput"),
        ],
        "elapsedMs": 12,
        "stdoutBytes": 300,
        "stderrBytes": 0,
    }

    assert validation_errors(receipt) == []
